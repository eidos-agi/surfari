//! Explicit redacted Browserbase lifecycle. The encrypted record is never opened here.
use crate::native::browser::{BrowserManager, WaitUntil};
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
    #[serde(default)]
    context_alias: Option<String>,
    #[serde(default)]
    current_url: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
struct ContextRec {
    id: String,
    alias: String,
    request_id: String,
    request_digest: String,
}
#[derive(Default, Deserialize, Serialize)]
struct State {
    version: u8,
    sessions: BTreeMap<String, Rec>,
    requests: BTreeMap<String, String>,
    #[serde(default)]
    contexts: BTreeMap<String, ContextRec>,
    #[serde(default)]
    context_requests: BTreeMap<String, String>,
    #[serde(default)]
    context_revocations: BTreeMap<String, String>,
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
            version: 3,
            ..Default::default()
        });
    }
    let mut s: State = serde_json::from_slice(&fs::read(p).map_err(|_| "State unreadable")?)
        .map_err(|_| "State invalid")?;
    if matches!(s.version, 1..=3) {
        s.version = 3;
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
fn safe_page_url(value: &str) -> Result<String, String> {
    let mut url = Url::parse(value).map_err(|_| "Unsafe page URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Unsafe page URL".into());
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}
fn output(r: &Rec, d: &str) -> Value {
    json!({"ok":true,"provider":"browserbase","session_id":r.id,"alias":r.alias,"status":r.status,"live_view_url":r.live_view_url,"context_alias":r.context_alias,"persistent_context":r.context_alias.is_some(),"current_url":r.current_url,"disposition":d})
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
fn request_digest(
    alias: Option<&str>,
    start: Option<&str>,
    ttl: u64,
    context_alias: Option<&str>,
) -> String {
    let canonical =
        json!({"alias":alias,"start_url":start,"ttl":ttl,"context_alias":context_alias})
            .to_string();
    hex::encode(Sha256::digest(canonical.as_bytes()))
}
fn context_request_digest(alias: &str) -> String {
    hex::encode(Sha256::digest(
        json!({"alias":alias}).to_string().as_bytes(),
    ))
}

fn context_revoke_digest(alias: &str) -> String {
    hex::encode(Sha256::digest(
        json!({"operation":"context.revoke","alias":alias})
            .to_string()
            .as_bytes(),
    ))
}
fn session_body(ttl: u64, start: Option<&str>, context_id: Option<&str>) -> Value {
    let mut body = json!({"keepAlive":true,"timeout":ttl});
    // Navigation is a CDP operation performed after session creation. Browserbase
    // accepts userMetadata, but it does not interpret it as a start URL.
    let _ = start;
    if let Some(id) = context_id {
        body["browserSettings"] = json!({"context":{"id":id,"persist":true}});
    }
    body
}
fn safe_control_url(value: &str) -> Result<&str, String> {
    let url = Url::parse(value).map_err(|_| "Invalid Browserbase control URL")?;
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    if url.scheme() == "wss"
        && (host == "browserbase.com" || host.ends_with(".browserbase.com"))
        && url.username().is_empty()
        && url.password().is_none()
    {
        Ok(value)
    } else {
        Err("Invalid Browserbase control URL".into())
    }
}
async fn navigate_start(raw: &Value, target: &str) -> Result<String, String> {
    let control_url = raw
        .get("connectUrl")
        .and_then(Value::as_str)
        .ok_or("Missing Browserbase control URL")?;
    let mut browser = BrowserManager::connect_cdp(safe_control_url(control_url)?).await?;
    browser.navigate(target, WaitUntil::None).await?;
    for _ in 0..40 {
        let current = browser.get_url().await.unwrap_or_default();
        if !current.is_empty() && current != "about:blank" {
            return Ok(current);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Err("Browserbase start navigation did not leave about:blank".into())
}
fn context_output(r: &ContextRec, disposition: &str) -> Value {
    json!({"ok":true,"provider":"browserbase","context_alias":r.alias,"persistent":true,"disposition":disposition})
}
async fn create_context(key: &str, args: &[String], p: &Path) -> Result<Value, String> {
    let (mut alias, mut rid) = (None, None);
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1).ok_or("Missing option value")?.clone();
        match args[i].as_str() {
            "--alias" => alias = Some(value),
            "--request-id" => rid = Some(value),
            _ => return Err("Unknown context create option".into()),
        }
        i += 2;
    }
    let alias = alias.ok_or("Context alias required")?;
    if !token(&alias) {
        return Err("Invalid context alias".into());
    }
    let rid = rid.unwrap_or_else(|| format!("bb-context-{}", now()));
    if !token(&rid) {
        return Err("Invalid request ID".into());
    }
    let mut state = load(p)?;
    let digest = context_request_digest(&alias);
    if let Some(prior_alias) = state.context_requests.get(&rid) {
        let prior = state
            .contexts
            .get(prior_alias)
            .ok_or("Inconsistent state")?;
        if prior.request_digest != digest {
            return Err("Request ID reused with changed parameters".into());
        }
        return Ok(context_output(prior, "replayed"));
    }
    if state.contexts.contains_key(&alias) {
        return Err("Context alias already exists".into());
    }
    let raw = crate::native::providers::browserbase_context_create(key).await?;
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| token(value))
        .ok_or("Invalid context ID")?
        .to_owned();
    let record = ContextRec {
        id,
        alias: alias.clone(),
        request_id: rid.clone(),
        request_digest: digest,
    };
    state.context_requests.insert(rid, alias.clone());
    state.contexts.insert(alias, record.clone());
    save(p, &state)?;
    Ok(context_output(&record, "created"))
}
fn context_status(alias: &str, p: &Path) -> Result<Value, String> {
    if !token(alias) {
        return Err("Invalid context alias".into());
    }
    let state = load(p)?;
    let record = state.contexts.get(alias).ok_or("Unknown context alias")?;
    Ok(context_output(record, "observed"))
}
fn context_list(p: &Path) -> Result<Value, String> {
    let state = load(p)?;
    let contexts: Vec<Value> = state
        .contexts
        .values()
        .map(|record| {
            let session_count = state
                .sessions
                .values()
                .filter(|session| session.context_alias.as_deref() == Some(&record.alias))
                .count();
            json!({
                "alias": record.alias,
                "persistent": true,
                "session_count": session_count
            })
        })
        .collect();
    Ok(json!({
        "ok": true,
        "provider": "browserbase",
        "contexts": contexts,
        "count": contexts.len()
    }))
}

async fn revoke_context(key: &str, args: &[String], p: &Path) -> Result<Value, String> {
    let alias = args.first().ok_or("Context alias required")?.clone();
    if !token(&alias) {
        return Err("Invalid context alias".into());
    }
    let mut rid = None;
    let mut i = 1;
    while i < args.len() {
        let value = args.get(i + 1).ok_or("Missing option value")?.clone();
        match args[i].as_str() {
            "--request-id" => rid = Some(value),
            _ => return Err("Unknown context revoke option".into()),
        }
        i += 2;
    }
    let rid = rid.unwrap_or_else(|| format!("bb-context-revoke-{}", now()));
    if !token(&rid) {
        return Err("Invalid request ID".into());
    }
    let mut state = load(p)?;
    let request_digest = context_revoke_digest(&alias);
    if let Some(prior_alias) = state.context_revocations.get(&rid) {
        if context_revoke_digest(prior_alias) != request_digest {
            return Err("Request ID reused with changed parameters".into());
        }
        return Ok(json!({
            "ok": true,
            "provider": "browserbase",
            "context_alias": prior_alias,
            "persistent": false,
            "disposition": "replayed"
        }));
    }
    if state.sessions.values().any(|session| {
        !session.released && session.context_alias.as_deref() == Some(alias.as_str())
    }) {
        return Err("Context has active sessions".into());
    }
    let record = state
        .contexts
        .get(&alias)
        .ok_or("Unknown context alias")?
        .clone();
    crate::native::providers::browserbase_context_delete(key, &record.id).await?;
    state.contexts.remove(&alias);
    state.context_revocations.insert(rid, alias.clone());
    save(p, &state)?;
    Ok(json!({
        "ok": true,
        "provider": "browserbase",
        "context_alias": alias,
        "persistent": false,
        "disposition": "revoked"
    }))
}
async fn create(key: &str, args: &[String], p: &Path) -> Result<Value, String> {
    let (mut alias, mut rid, mut start, mut context_alias, mut ttl) =
        (None, None, None, None, 900u64);
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).ok_or("Missing option value")?.clone();
        match args[i].as_str() {
            "--alias" => alias = Some(v),
            "--request-id" => rid = Some(v),
            "--start-url" => start = Some(v),
            "--context" => context_alias = Some(v),
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
    if context_alias.as_deref().is_some_and(|v| !token(v)) {
        return Err("Invalid context alias".into());
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
    let digest = request_digest(
        alias.as_deref(),
        start.as_deref(),
        ttl,
        context_alias.as_deref(),
    );
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
    let context_id = context_alias
        .as_ref()
        .map(|name| {
            s.contexts
                .get(name)
                .map(|record| record.id.as_str())
                .ok_or("Unknown context alias")
        })
        .transpose()?;
    let body = session_body(ttl, start.as_deref(), context_id);
    let raw = crate::native::providers::browserbase_create(key, &body).await?;
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .filter(|v| token(v))
        .ok_or("Invalid session ID")?
        .to_owned();
    let current_url = if let Some(url) = start.as_deref() {
        match navigate_start(&raw, url).await {
            Ok(current) => Some(current),
            Err(error) => {
                let _ = crate::native::providers::browserbase_release(key, &id).await;
                return Err(error);
            }
        }
    } else {
        None
    };
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
        context_alias,
        current_url,
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
async fn inspect(key: &str, k: &str, p: &Path) -> Result<Value, String> {
    let state = load(p)?;
    let record = find(&state, k)?.clone();
    let raw = crate::native::providers::browserbase_status(key, &record.id).await?;
    let control_url = raw
        .get("connectUrl")
        .and_then(Value::as_str)
        .ok_or("Missing Browserbase control URL")?;
    let browser = BrowserManager::connect_cdp(safe_control_url(control_url)?).await?;
    let current_url = safe_page_url(&browser.get_url().await?)?;
    let title: String = browser.get_title().await?.chars().take(160).collect();
    Ok(json!({
        "ok": true,
        "provider": "browserbase",
        "session_id": record.id,
        "alias": record.alias,
        "context_alias": record.context_alias,
        "current_url": current_url,
        "title": title,
        "disposition": "inspected"
    }))
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
    let state_path = path();
    let r = match args.first().map(String::as_str) {
        Some("context")
            if args.get(1).map(String::as_str) == Some("status") && args.len() == 3 =>
        {
            context_status(&args[2], &state_path)
        }
        Some("context")
            if args.get(1).map(String::as_str) == Some("list") && args.len() == 2 =>
        {
            context_list(&state_path)
        }
        _ => tokio::runtime::Runtime::new().unwrap().block_on(async {
            let key = credential()?;
            match args.first().map(String::as_str) {
                Some("context") if args.get(1).map(String::as_str) == Some("create") => {
                    create_context(&key, &args[2..], &state_path).await
                }
                Some("context") if args.get(1).map(String::as_str) == Some("revoke") => {
                    revoke_context(&key, &args[2..], &state_path).await
                }
            Some("create") => create(&key, &args[1..], &path()).await,
            Some("status") if args.len() == 2 => status(&key, &args[1], &path()).await,
            Some("inspect") if args.len() == 2 => inspect(&key, &args[1], &path()).await,
            Some("release") if args.len() == 2 => release(&key, &args[1], &path()).await,
            _ => Err(
                "Usage: surfari browserbase <context create|context list|context status|context revoke|create|status|inspect|release>"
                    .into(),
            ),
            }
        }),
    };
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
        assert_eq!(
            safe_page_url("https://app.slack.com/client/T1/C1?token=secret#fragment").unwrap(),
            "https://app.slack.com/client/T1/C1"
        );
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
            context_alias: None,
            current_url: None,
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
        let digest = request_digest(Some("proof"), None, 900, None);
        let record = Rec {
            id: "session_1".into(),
            alias: Some("proof".into()),
            status: "RUNNING".into(),
            live_view_url: Some("https://browserbase.com/sessions/session_1".into()),
            request_id: "request_1".into(),
            request_digest: digest,
            released: false,
            context_alias: None,
            current_url: None,
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
    #[test]
    fn context_body_is_persistent_and_output_is_redacted() {
        let body = session_body(
            900,
            Some("https://app.slack.com"),
            Some("context_secret_id"),
        );
        assert_eq!(
            body["browserSettings"]["context"]["id"],
            "context_secret_id"
        );
        assert_eq!(body["browserSettings"]["context"]["persist"], true);
        assert!(body.get("userMetadata").is_none());
        let record = ContextRec {
            id: "context_secret_id".into(),
            alias: "slack-eidos".into(),
            request_id: "request_1".into(),
            request_digest: context_request_digest("slack-eidos"),
        };
        let visible = context_output(&record, "created").to_string();
        assert!(visible.contains("slack-eidos"));
        assert!(!visible.contains("context_secret_id"));
    }
    #[test]
    fn browserbase_control_url_is_narrowly_validated() {
        assert!(safe_control_url("wss://connect.browserbase.com?apiKey=transient").is_ok());
        assert!(safe_control_url("https://connect.browserbase.com").is_err());
        assert!(safe_control_url("wss://evil.test").is_err());
    }
    #[tokio::test]
    async fn context_replay_is_network_free_and_changed_request_denies() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("state.json");
        let record = ContextRec {
            id: "context_secret_id".into(),
            alias: "slack-eidos".into(),
            request_id: "context_request".into(),
            request_digest: context_request_digest("slack-eidos"),
        };
        let mut state = State {
            version: 2,
            ..Default::default()
        };
        state.contexts.insert(record.alias.clone(), record);
        state
            .context_requests
            .insert("context_request".into(), "slack-eidos".into());
        save(&state_path, &state).unwrap();
        let same = vec![
            "--alias".into(),
            "slack-eidos".into(),
            "--request-id".into(),
            "context_request".into(),
        ];
        assert_eq!(
            create_context("not-used", &same, &state_path)
                .await
                .unwrap()["disposition"],
            "replayed"
        );
        let changed = vec![
            "--alias".into(),
            "other".into(),
            "--request-id".into(),
            "context_request".into(),
        ];
        assert!(create_context("not-used", &changed, &state_path)
            .await
            .unwrap_err()
            .contains("changed"));
        let durable = fs::read_to_string(&state_path).unwrap();
        assert!(durable.contains("context_secret_id"));
        for forbidden in ["not-used", "connectUrl", "signingKey", "wsUrl"] {
            assert!(!durable.contains(forbidden));
        }
    }
    #[test]
    fn version_one_state_migrates_without_contexts() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("state.json");
        fs::write(&state_path, r#"{"version":1,"sessions":{},"requests":{}}"#).unwrap();
        let state = load(&state_path).unwrap();
        assert_eq!(state.version, 3);
        assert!(state.contexts.is_empty());
    }
    #[test]
    fn context_library_lists_aliases_without_context_ids() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("state.json");
        let mut state = State {
            version: 2,
            ..Default::default()
        };
        state.contexts.insert(
            "slack-eidos".into(),
            ContextRec {
                id: "context_secret_id".into(),
                alias: "slack-eidos".into(),
                request_id: "request_1".into(),
                request_digest: context_request_digest("slack-eidos"),
            },
        );
        save(&state_path, &state).unwrap();
        let visible = context_list(&state_path).unwrap().to_string();
        assert!(visible.contains("slack-eidos"));
        assert!(!visible.contains("context_secret_id"));
    }

    #[tokio::test]
    async fn context_revoke_replay_is_network_free_changed_request_denies_and_redacts() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("state.json");
        let mut state = State {
            version: 3,
            ..Default::default()
        };
        state
            .context_revocations
            .insert("revoke_request".into(), "slack-disposable".into());
        save(&state_path, &state).unwrap();
        let same = vec![
            "slack-disposable".into(),
            "--request-id".into(),
            "revoke_request".into(),
        ];
        let replay = revoke_context("not-used", &same, &state_path)
            .await
            .unwrap();
        assert_eq!(replay["disposition"], "replayed");
        assert_eq!(replay["persistent"], false);
        let changed = vec![
            "other".into(),
            "--request-id".into(),
            "revoke_request".into(),
        ];
        assert!(revoke_context("not-used", &changed, &state_path)
            .await
            .unwrap_err()
            .contains("changed"));
        let durable = fs::read_to_string(&state_path).unwrap();
        for forbidden in ["not-used", "api_key", "connectUrl", "signingKey", "wsUrl"] {
            assert!(!durable.contains(forbidden));
        }
    }
}
