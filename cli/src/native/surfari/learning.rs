use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use url::Url;

pub struct CandidateInput<'a> {
    pub action_id: &'a str,
    pub command_id: &'a str,
    pub action: &'a str,
    pub browser_session: &'a str,
    pub use_id: Option<&'a str>,
    pub learning_context: &'a Value,
    pub command_metadata: &'a Value,
    pub surfari_context: &'a Value,
    pub browser_anchor: &'a Value,
    pub response: &'a Value,
}

pub fn load_context(use_id: Option<&str>, browser_session: &str, cmd: &Value) -> Value {
    let domain = domain_from_command(cmd);
    let metadata_found = use_id
        .and_then(read_use_metadata)
        .map(|_| true)
        .unwrap_or(false);
    json!({
        "use_id": use_id,
        "browser_session": browser_session,
        "domain": domain,
        "metadata_found": metadata_found,
    })
}

pub fn command_metadata(cmd: &Value) -> Value {
    let action = cmd.get("action").and_then(|v| v.as_str()).unwrap_or("");
    json!({
        "domain": domain_from_command(cmd),
        "url": safe_url_from_command(cmd),
        "selector": safe_string(cmd, "selector"),
        "ref": safe_string(cmd, "ref"),
        "screenshot_path": screenshot_path_from_command(action, cmd),
    })
}

pub fn candidate_from_response(input: CandidateInput<'_>) -> Value {
    let success = input
        .response
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    json!({
        "event_type": "learning_candidate",
        "action_id": input.action_id,
        "command_id": input.command_id,
        "action": input.action,
        "browser_session": input.browser_session,
        "use_id": input.use_id,
        "domain": input.learning_context.get("domain").cloned().unwrap_or(Value::Null),
        "url": response_url(input.response)
            .or_else(|| input.command_metadata.get("url").cloned())
            .unwrap_or(Value::Null),
        "selector": input.command_metadata.get("selector").cloned().unwrap_or(Value::Null),
        "ref": input.command_metadata.get("ref").cloned().unwrap_or(Value::Null),
        "title": title_summary(input.action, input.response).unwrap_or(Value::Null),
        "screenshot_path": response_screenshot_path(input.action, input.response)
            .or_else(|| input.command_metadata.get("screenshot_path").cloned())
            .unwrap_or(Value::Null),
        "surfari_context": input.surfari_context,
        "browser_anchor": input.browser_anchor,
        "success": success,
        "error_type": input
            .response
            .get("error")
            .and_then(|v| v.as_str())
            .map(|_| "command_error"),
        "result_shape": result_shape(input.response.get("data").unwrap_or(&Value::Null)),
    })
}

fn read_use_metadata(use_id: &str) -> Option<Value> {
    let path = dirs::home_dir()?
        .join(".cache")
        .join("surfari")
        .join("uses")
        .join(use_id)
        .join("metadata.json");
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn domain_from_command(cmd: &Value) -> Option<String> {
    let url = cmd
        .get("url")
        .or_else(|| cmd.get("href"))
        .and_then(|v| v.as_str())?;
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_string()))
}

fn safe_url_from_command(cmd: &Value) -> Option<Value> {
    let raw = cmd
        .get("url")
        .or_else(|| cmd.get("href"))
        .and_then(|v| v.as_str())?;
    sanitize_url(raw).map(Value::from)
}

fn sanitize_url(raw: &str) -> Option<String> {
    let mut parsed = Url::parse(raw).ok()?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

fn safe_string(value: &Value, key: &str) -> Option<Value> {
    value.get(key).and_then(|v| v.as_str()).map(Value::from)
}

fn screenshot_path_from_command(action: &str, cmd: &Value) -> Option<Value> {
    if !matches!(action, "screenshot" | "diff_screenshot") {
        return None;
    }
    safe_string(cmd, "path")
}

fn response_url(response: &Value) -> Option<Value> {
    response
        .get("data")
        .and_then(|data| data.get("url").or_else(|| data.get("currentUrl")))
        .and_then(|v| v.as_str())
        .and_then(sanitize_url)
        .map(Value::from)
}

fn response_screenshot_path(action: &str, response: &Value) -> Option<Value> {
    if !matches!(action, "screenshot" | "diff_screenshot") {
        return None;
    }
    response
        .get("data")
        .and_then(|data| {
            data.get("path")
                .or_else(|| data.get("screenshotPath"))
                .or_else(|| data.get("output"))
        })
        .and_then(|v| v.as_str())
        .map(Value::from)
}

fn title_summary(action: &str, response: &Value) -> Option<Value> {
    let title = response
        .get("data")
        .and_then(|data| data.get("title").or_else(|| data.get("currentTitle")))
        .and_then(|v| v.as_str())
        .or_else(|| {
            if action == "title" {
                response.get("data").and_then(|v| v.as_str())
            } else {
                None
            }
        })?;
    Some(json!({
        "byte_length": title.len(),
        "sha256": hash_string(title),
    }))
}

fn result_shape(value: &Value) -> Value {
    match value {
        Value::Null => json!({"type": "null"}),
        Value::Bool(_) => json!({"type": "bool"}),
        Value::Number(_) => json!({"type": "number"}),
        Value::String(s) => json!({"type": "string", "byte_length": s.len()}),
        Value::Array(items) => json!({"type": "array", "len": items.len()}),
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            json!({"type": "object", "keys": keys})
        }
    }
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
    fn context_extracts_domain_from_command() {
        let context = load_context(
            None,
            "session",
            &json!({
                "action": "navigate",
                "url": "https://example.com/path?token=secret"
            }),
        );

        assert_eq!(context["domain"], "example.com");
        assert_eq!(context["metadata_found"], false);
    }

    #[test]
    fn candidate_records_shape_not_raw_text() {
        let context = json!({"domain": "example.com"});
        let metadata = json!({
            "url": "https://example.com/path",
            "selector": "#main",
            "ref": "@e1",
            "screenshot_path": null
        });
        let response = json!({"success": true, "data": "Visible page text"});
        let candidate = candidate_from_response(CandidateInput {
            action_id: "a1",
            command_id: "cmd1",
            action: "snapshot",
            browser_session: "session",
            use_id: Some("use_1"),
            learning_context: &context,
            command_metadata: &metadata,
            surfari_context: &json!({"context_id": "eidos"}),
            browser_anchor: &json!({"active_tab_id": "t1"}),
            response: &response,
        });
        let text = serde_json::to_string(&candidate).unwrap();

        assert!(!text.contains("Visible page text"));
        assert_eq!(candidate["result_shape"]["type"], "string");
        assert_eq!(candidate["domain"], "example.com");
        assert_eq!(candidate["selector"], "#main");
        assert_eq!(candidate["surfari_context"]["context_id"], "eidos");
    }

    #[test]
    fn candidate_records_sanitized_url_and_screenshot_path() {
        let context = json!({"domain": "example.com"});
        let metadata = command_metadata(&json!({
            "action": "screenshot",
            "url": "https://example.com/path?token=secret#frag",
            "path": "/tmp/page.png"
        }));
        let response = json!({
            "success": true,
            "data": {
                "url": "https://example.com/after?token=secret",
                "title": "Example title",
                "path": "/tmp/page.png"
            }
        });
        let candidate = candidate_from_response(CandidateInput {
            action_id: "a1",
            command_id: "cmd1",
            action: "screenshot",
            browser_session: "session",
            use_id: Some("use_1"),
            learning_context: &context,
            command_metadata: &metadata,
            surfari_context: &json!({"context_id": "eidos"}),
            browser_anchor: &json!({"active_tab_id": "t1"}),
            response: &response,
        });
        let text = serde_json::to_string(&candidate).unwrap();

        assert!(!text.contains("secret"));
        assert!(!text.contains("Example title"));
        assert_eq!(candidate["url"], "https://example.com/after");
        assert_eq!(candidate["screenshot_path"], "/tmp/page.png");
        assert_eq!(candidate["title"]["byte_length"], 13);
    }
}
