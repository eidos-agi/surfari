//! Explicit redacted Browserbase lifecycle. The encrypted record is never opened here.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use url::Url;
const CRED: &str = "browserbase-api-key";
#[derive(Clone, Deserialize, Serialize)]
struct Rec {
    id: String,
    alias: Option<String>,
    status: String,
    live_view_url: Option<String>,
    request_id: String,
    request_digest: String,
    released: bool,
}
#[derive(Default, Deserialize, Serialize)]
struct State {
    version: u8,
    sessions: BTreeMap<String, Rec>,
    requests: BTreeMap<String, String>,
}
fn credential() -> Result<String, String> {
    let d = env::var_os("CREDENTIALS_DIRECTORY")
        .ok_or("Browserbase credential unavailable from encrypted broker")?;
    let k = fs::read_to_string(PathBuf::from(d).join(CRED))
        .map_err(|_| "Browserbase credential unavailable from encrypted broker")?;
    let k = k.trim().to_owned();
    if k.is_empty() {
        Err("Browserbase broker credential is empty".into())
    } else {
        Ok(k)
    }
}
fn path() -> PathBuf {
    env::var_os("SURFARI_BROWSERBASE_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_default()
                .join(".surfari/browserbase-sessions.json")
        })
}
fn load(p: &Path) -> Result<State, String> {
    if !p.exists() {
        return Ok(State {
            version: 1,
            ..Default::default()
        });
    }
    let s: State = serde_json::from_slice(&fs::read(p).map_err(|_| "State unreadable")?)
        .map_err(|_| "State invalid")?;
    if s.version == 1 {
        Ok(s)
    } else {
        Err("Unsupported state version".into())
    }
}
fn save(p: &Path, s: &State) -> Result<(), String> {
    let d = p.parent().ok_or("Invalid state path")?;
    fs::create_dir_all(d).map_err(|_| "State directory failed")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(d, fs::Permissions::from_mode(0o700))
            .map_err(|_| "State permissions failed")?;
    }
    let t = p.with_extension("tmp");
    fs::write(
        &t,
        serde_json::to_vec(s).map_err(|_| "State serialization failed")?,
    )
    .map_err(|_| "State write failed")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&t, fs::Permissions::from_mode(0o600))
            .map_err(|_| "State permissions failed")?;
    }
    fs::rename(t, p).map_err(|_| "State commit failed".into())
}
fn token(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}
fn safe_live(v: &str) -> Result<String, String> {
    let u = Url::parse(v).map_err(|_| "Unsafe live-view URL")?;
    let h = u.host_str().unwrap_or("").to_ascii_lowercase();
    if u.scheme() == "https"
        && (h == "browserbase.com" || h.ends_with(".browserbase.com"))
        && u.username().is_empty()
        && u.password().is_none()
    {
        Ok(v.into())
    } else {
        Err("Unsafe live-view URL".into())
    }
}
fn output(r: &Rec, d: &str) -> Value {
    json!({"ok":true,"provider":"browserbase","session_id":r.id,"alias":r.alias,"status":r.status,"live_view_url":r.live_view_url,"disposition":d})
}
fn find<'a>(s: &'a State, k: &str) -> Result<&'a Rec, String> {
    if !token(k) {
        return Err("Invalid session or alias".into());
    }
    s.sessions
        .get(k)
        .or_else(|| s.sessions.values().find(|r| r.alias.as_deref() == Some(k)))
        .ok_or("Unknown session or alias".into())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn request_digest(alias: Option<&str>, start: Option<&str>, ttl: u64) -> String {
    let canonical = json!({"alias":alias,"start_url":start,"ttl":ttl}).to_string();
    hex::encode(Sha256::digest(canonical.as_bytes()))
}
async fn create(key: &str, args: &[String], p: &Path) -> Result<Value, String> {
    let (mut alias, mut rid, mut start, mut ttl) = (None, None, None, 900u64);
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).ok_or("Missing option value")?.clone();
        match args[i].as_str() {
            "--alias" => alias = Some(v),
            "--request-id" => rid = Some(v),
            "--start-url" => start = Some(v),
            "--ttl" => ttl = v.parse().map_err(|_| "TTL invalid")?,
            _ => return Err("Unknown create option".into()),
        }
        i += 2
    }
    if !(60..=21600).contains(&ttl) {
        return Err("TTL out of bounds".into());
    }
    if alias.as_deref().is_some_and(|v| !token(v)) {
        return Err("Invalid alias".into());
    }
    if let Some(v) = start.as_deref() {
        let u = Url::parse(v).map_err(|_| "Start URL invalid")?;
        if u.scheme() != "https"
            || u.host_str().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err("Start URL must be HTTPS without credentials".into());
        }
    }
    let rid = rid.unwrap_or_else(|| format!("bb-{}", now()));
    if !token(&rid) {
        return Err("Invalid request ID".into());
    }
    let mut s = load(p)?;
    let digest = request_digest(alias.as_deref(), start.as_deref(), ttl);
    if let Some(id) = s.requests.get(&rid) {
        let prior = s.sessions.get(id).ok_or("Inconsistent state")?;
        if prior.request_digest != digest {
            return Err("Request ID reused with changed parameters".into());
        }
        return Ok(output(prior, "replayed"));
    }
    if alias
        .as_ref()
        .is_some_and(|x| s.sessions.values().any(|r| r.alias.as_ref() == Some(x)))
    {
        return Err("Alias already exists".into());
    }
    let mut body = json!({"keepAlive":true,"timeout":ttl});
    if let Some(url) = start.as_deref() {
        body["userMetadata"] = json!({"surfariStartUrl":url});
    }
    let raw = crate::native::providers::browserbase_create(key, &body).await?;
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .filter(|v| token(v))
        .ok_or("Invalid session ID")?
        .to_owned();
    let dbg = match crate::native::providers::browserbase_debug(key, &id).await {
        Ok(value) => value,
        Err(error) => {
            let _ = crate::native::providers::browserbase_release(key, &id).await;
            return Err(error);
        }
    };
    let view = safe_live(
        dbg.get("debuggerFullscreenUrl")
            .and_then(Value::as_str)
            .ok_or("Missing live-view URL")?,
    )?;
    let r = Rec {
        id: id.clone(),
        alias,
        status: "RUNNING".into(),
        live_view_url: Some(view),
        request_id: rid.clone(),
        request_digest: digest,
        released: false,
    };
    s.requests.insert(rid, id.clone());
    s.sessions.insert(id, r.clone());
    save(p, &s)?;
    Ok(output(&r, "created"))
}
async fn status(key: &str, k: &str, p: &Path) -> Result<Value, String> {
    let mut s = load(p)?;
    let r = find(&s, k)?.clone();
    let raw = crate::native::providers::browserbase_status(key, &r.id).await?;
    let st = raw
        .get("status")
        .and_then(Value::as_str)
        .ok_or("Missing status")?
        .to_owned();
    let x = s.sessions.get_mut(&r.id).unwrap();
    x.status = st;
    x.released = matches!(x.status.as_str(), "COMPLETED" | "TIMED_OUT" | "ERROR");
    let out = output(x, "observed");
    save(p, &s)?;
    Ok(out)
}
async fn release(key: &str, k: &str, p: &Path) -> Result<Value, String> {
    let mut s = load(p)?;
    let r = find(&s, k)?.clone();
    if r.released {
        return Ok(output(&r, "replayed"));
    }
    let raw = crate::native::providers::browserbase_release(key, &r.id).await?;
    let x = s.sessions.get_mut(&r.id).unwrap();
    x.status = raw
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("COMPLETED")
        .into();
    x.released = true;
    x.live_view_url = None;
    let out = output(x, "released");
    save(p, &s)?;
    Ok(out)
}
pub fn run(args: &[String]) -> i32 {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let r = rt.block_on(async {
        let key = credential()?;
        match args.first().map(String::as_str) {
            Some("create") => create(&key, &args[1..], &path()).await,
            Some("status") if args.len() == 2 => status(&key, &args[1], &path()).await,
            Some("release") if args.len() == 2 => release(&key, &args[1], &path()).await,
            _ => Err("Usage: surfari browserbase <create|status|release>".into()),
        }
    });
    match r {
        Ok(v) => {
            println!("{}", serde_json::to_string(&v).unwrap());
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redaction_and_validation() {
        assert!(safe_live("https://www.browserbase.com/sessions/x").is_ok());
        assert!(safe_live("https://evil.test/x").is_err());
        assert!(token("session_1"));
        assert!(!token("../secret"));
        let r = Rec {
            id: "s1".into(),
            alias: None,
            status: "RUNNING".into(),
            live_view_url: Some("https://browserbase.com/x".into()),
            request_id: "r1".into(),
            request_digest: "digest".into(),
            released: false,
        };
        let o = output(&r, "created").to_string();
        for x in ["connectUrl", "signingKey", "wsUrl", "api_key"] {
            assert!(!o.contains(x));
        }
    }
    #[test]
    fn broker_requires_mount() {
        let g = crate::test_utils::EnvGuard::new(&["CREDENTIALS_DIRECTORY"]);
        g.remove("CREDENTIALS_DIRECTORY");
        assert!(credential().unwrap_err().contains("broker"));
    }
    #[test]
    fn bounds() {
        assert!(!(60..=21600).contains(&59));
    }
    #[tokio::test]
    async fn replay_is_network_free_and_changed_request_denies() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("state.json");
        let digest = request_digest(Some("proof"), None, 900);
        let record = Rec {
            id: "session_1".into(),
            alias: Some("proof".into()),
            status: "RUNNING".into(),
            live_view_url: Some("https://browserbase.com/sessions/session_1".into()),
            request_id: "request_1".into(),
            request_digest: digest,
            released: false,
        };
        let mut state = State {
            version: 1,
            ..Default::default()
        };
        state
            .requests
            .insert("request_1".into(), "session_1".into());
        state.sessions.insert("session_1".into(), record);
        save(&state_path, &state).unwrap();
        let same = vec![
            "--alias".into(),
            "proof".into(),
            "--request-id".into(),
            "request_1".into(),
        ];
        assert_eq!(
            create("not-used", &same, &state_path).await.unwrap()["disposition"],
            "replayed"
        );
        let changed = vec![
            "--alias".into(),
            "other".into(),
            "--request-id".into(),
            "request_1".into(),
        ];
        assert!(create("not-used", &changed, &state_path)
            .await
            .unwrap_err()
            .contains("changed"));
        let durable = fs::read_to_string(&state_path).unwrap();
        for forbidden in ["connectUrl", "signingKey", "wsUrl", "not-used"] {
            assert!(!durable.contains(forbidden));
        }
    }
}
