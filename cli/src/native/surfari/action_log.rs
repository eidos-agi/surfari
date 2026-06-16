use serde_json::{json, Value};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const ACTION_LOG_PATH_ENV: &str = "SURFARI_ACTION_LOG_PATH";
pub const USE_ID_ENV: &str = "SURFARI_USE_ID";

pub fn current_use_id() -> Option<String> {
    env::var(USE_ID_ENV)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn home_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(env::temp_dir)
        .join(".cache")
        .join("surfari")
}

pub fn resolve_log_path(use_id: Option<&str>) -> PathBuf {
    if let Ok(path) = env::var(ACTION_LOG_PATH_ENV) {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Some(use_id) = use_id.filter(|s| !s.trim().is_empty()) {
        return home_cache_dir()
            .join("uses")
            .join(use_id)
            .join("browser-actions.jsonl");
    }
    let date = OffsetDateTime::now_utc().date().to_string();
    home_cache_dir()
        .join("browser-actions")
        .join(format!("{date}.jsonl"))
}

pub fn resolve_learning_candidates_path(use_id: Option<&str>) -> Option<PathBuf> {
    use_id.filter(|s| !s.trim().is_empty()).map(|id| {
        home_cache_dir()
            .join("uses")
            .join(id)
            .join("learning-candidates.jsonl")
    })
}

pub fn append_event(path: &Path, event: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut payload = match event {
        Value::Object(map) => Value::Object(map.clone()),
        other => json!({ "event": other }),
    };
    if let Value::Object(map) = &mut payload {
        map.insert("timestamp".to_string(), json!(timestamp()));
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, &payload)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn timestamp() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| OffsetDateTime::now_utc().unix_timestamp().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;
    use serde_json::json;

    #[test]
    fn log_path_override_wins() {
        let guard = EnvGuard::new(&[ACTION_LOG_PATH_ENV, USE_ID_ENV]);
        guard.set(ACTION_LOG_PATH_ENV, "/tmp/surfari-actions.jsonl");
        guard.set(USE_ID_ENV, "use_123");

        assert_eq!(
            resolve_log_path(current_use_id().as_deref()),
            PathBuf::from("/tmp/surfari-actions.jsonl")
        );
    }

    #[test]
    fn log_path_uses_use_id_when_present() {
        let guard = EnvGuard::new(&[ACTION_LOG_PATH_ENV, USE_ID_ENV, "HOME"]);
        let temp = tempfile::tempdir().unwrap();
        guard.remove(ACTION_LOG_PATH_ENV);
        guard.set("HOME", temp.path().to_str().unwrap());

        assert_eq!(
            resolve_log_path(Some("use_abc")),
            temp.path()
                .join(".cache")
                .join("surfari")
                .join("uses")
                .join("use_abc")
                .join("browser-actions.jsonl")
        );
    }

    #[test]
    fn log_path_daily_fallback_without_use_id() {
        let guard = EnvGuard::new(&[ACTION_LOG_PATH_ENV, USE_ID_ENV, "HOME"]);
        let temp = tempfile::tempdir().unwrap();
        guard.remove(ACTION_LOG_PATH_ENV);
        guard.remove(USE_ID_ENV);
        guard.set("HOME", temp.path().to_str().unwrap());

        let path = resolve_log_path(None);

        assert!(path.starts_with(
            temp.path()
                .join(".cache")
                .join("surfari")
                .join("browser-actions")
        ));
        assert_eq!(path.extension().and_then(|s| s.to_str()), Some("jsonl"));
    }

    #[test]
    fn append_event_writes_jsonl() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("actions.jsonl");

        append_event(&path, &json!({"event_type": "action_started"})).unwrap();

        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"event_type\":\"action_started\""));
        assert!(text.contains("\"timestamp\""));
    }
}
