use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const SECRET_ACTIONS: &[&str] = &[
    "fill",
    "type",
    "keyboard",
    "keyboard_type",
    "inserttext",
    "cookies_set",
    "storage_set",
    "headers",
    "credentials",
    "credentials_set",
    "auth_login",
    "auth_save",
    "setvalue",
];

const SECRET_KEY_PARTS: &[&str] = &[
    "authorization",
    "cookie",
    "credentials",
    "header",
    "mfa",
    "otp",
    "password",
    "proxy",
    "recovery",
    "secret",
    "token",
    "value",
];

const SAFE_SELECTOR_KEYS: &[&str] = &[
    "action",
    "id",
    "selector",
    "ref",
    "role",
    "name",
    "path",
    "url",
    "timeout",
    "state",
    "waitUntil",
];

pub fn safety_class(action: &str) -> &'static str {
    if SECRET_ACTIONS.contains(&action) {
        "secret_bearing"
    } else {
        "standard"
    }
}

pub fn redaction_status(cmd: &Value) -> &'static str {
    let action = cmd.get("action").and_then(|v| v.as_str()).unwrap_or("");
    if SECRET_ACTIONS.contains(&action) || contains_sensitive_key(cmd) {
        "redacted"
    } else {
        "not_required"
    }
}

pub fn redact_command(cmd: &Value) -> Value {
    let action = cmd.get("action").and_then(|v| v.as_str()).unwrap_or("");
    redact_value(cmd, SECRET_ACTIONS.contains(&action), None)
}

pub fn summarize_response(response: &Value) -> Value {
    let success = response
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut summary = Map::new();
    summary.insert("success".to_string(), json!(success));
    if let Some(error) = response.get("error").and_then(|v| v.as_str()) {
        summary.insert("error_type".to_string(), json!("command_error"));
        summary.insert("error_hash".to_string(), json!(hash_string(error)));
        summary.insert("error_bytes".to_string(), json!(error.len()));
    }
    if let Some(data) = response.get("data") {
        summary.insert("data".to_string(), summarize_data(data));
    }
    Value::Object(summary)
}

fn summarize_data(data: &Value) -> Value {
    match data {
        Value::Null | Value::Bool(_) | Value::Number(_) => data.clone(),
        Value::String(s) => json!({
            "type": "string",
            "byte_length": s.len(),
            "sha256": hash_string(s),
        }),
        Value::Array(items) => json!({
            "type": "array",
            "len": items.len(),
        }),
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            json!({
                "type": "object",
                "keys": keys,
            })
        }
    }
}

fn redact_value(value: &Value, secret_action: bool, key: Option<&str>) -> Value {
    if should_redact_key(key) {
        return redacted_leaf(value);
    }
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                let child_secret_action =
                    secret_action && !SAFE_SELECTOR_KEYS.contains(&k.as_str());
                if should_redact_key(Some(k)) || child_secret_action {
                    out.insert(k.clone(), redacted_leaf(v));
                } else {
                    out.insert(k.clone(), redact_value(v, false, Some(k)));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            if secret_action {
                return redacted_leaf(value);
            }
            Value::Array(
                items
                    .iter()
                    .map(|item| redact_value(item, false, key))
                    .collect(),
            )
        }
        _ if secret_action => redacted_leaf(value),
        _ => value.clone(),
    }
}

fn redacted_leaf(value: &Value) -> Value {
    let encoded = serde_json::to_string(value).unwrap_or_default();
    json!({
        "redacted": true,
        "byte_length": encoded.len(),
        "sha256": hash_string(&encoded),
    })
}

fn contains_sensitive_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, value)| should_redact_key(Some(key)) || contains_sensitive_key(value)),
        Value::Array(items) => items.iter().any(contains_sensitive_key),
        _ => false,
    }
}

fn should_redact_key(key: Option<&str>) -> bool {
    key.map(|k| {
        let lower = k.to_ascii_lowercase();
        SECRET_KEY_PARTS.iter().any(|part| lower.contains(part))
    })
    .unwrap_or(false)
}

fn hash_string(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_fill_value() {
        let cmd = json!({
            "id": "1",
            "action": "fill",
            "selector": "#password",
            "value": "hunter2"
        });

        let redacted = redact_command(&cmd);
        let text = serde_json::to_string(&redacted).unwrap();

        assert!(!text.contains("hunter2"));
        assert_eq!(redacted["selector"], "#password");
        assert_eq!(redacted["value"]["redacted"], true);
    }

    #[test]
    fn redacts_cookies_headers_tokens_and_storage_values() {
        let cmd = json!({
            "id": "2",
            "action": "headers",
            "headers": {"Authorization": "Bearer secret-token", "Cookie": "sid=secret"},
            "storageValue": "secret-storage"
        });

        let text = serde_json::to_string(&redact_command(&cmd)).unwrap();

        assert!(!text.contains("secret-token"));
        assert!(!text.contains("sid=secret"));
        assert!(!text.contains("secret-storage"));
    }

    #[test]
    fn response_summary_does_not_include_raw_page_text() {
        let response = json!({
            "id": "1",
            "success": true,
            "data": "Example Domain page text"
        });

        let summary = summarize_response(&response);
        let text = serde_json::to_string(&summary).unwrap();

        assert!(!text.contains("Example Domain page text"));
        assert_eq!(summary["data"]["type"], "string");
        assert_eq!(summary["data"]["byte_length"], 24);
    }
}
