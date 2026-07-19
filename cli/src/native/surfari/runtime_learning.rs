use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub const RULES_PATH_ENV: &str = "SURFARI_RUNTIME_LEARNINGS";
pub const EVENTS_PATH_ENV: &str = "SURFARI_RUNTIME_LEARNING_EVENTS";
const MAX_RULE_BYTES: u64 = 256 * 1024;
const MAX_GUIDANCE_BYTES: usize = 1000;
const MAX_SEMANTIC_RULES: usize = 500;

fn surfari_home(kind: &str, filename: &str) -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(kind).join("surfari").join(filename))
}

fn rules_path() -> Option<PathBuf> {
    env_path(RULES_PATH_ENV).or_else(|| surfari_home(".config", "runtime-learnings.json"))
}

fn events_path() -> Option<PathBuf> {
    env_path(EVENTS_PATH_ENV)
        .or_else(|| surfari_home(".local/state", "runtime-learning-events.jsonl"))
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
}

pub fn latest_domain(log_path: &Path, browser_session: &str) -> Option<String> {
    let text = fs::read_to_string(log_path).ok()?;
    text.lines().rev().take(500).find_map(|line| {
        let event: Value = serde_json::from_str(line).ok()?;
        if event.get("browser_session").and_then(Value::as_str) != Some(browser_session) {
            return None;
        }
        event
            .pointer("/learning_context/domain")
            .and_then(Value::as_str)
            .map(str::to_owned)
    })
}

pub fn apply(
    response: &mut Value,
    domain: Option<&str>,
    browser_session: &str,
    use_id: Option<&str>,
) {
    let Some(error) = response
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let error_digest = digest(&error);
    let observed_tags = classify(&error);
    let mut disposition = "no_store";
    let mut selected: Option<(Value, &'static str, u64)> = None;

    if let Some(document) = load_rules() {
        disposition = "no_match";
        if document.get("schema_version").and_then(Value::as_u64) == Some(1) {
            let rules = document
                .get("rules")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            selected = select_rule(rules, domain, &observed_tags);
            if selected.is_some() {
                disposition = "matched";
            }
        } else {
            disposition = "schema_rejected";
        }
    }

    let (rule_id, rule_version, match_method) = if let Some((rule, method, score)) = selected {
        let guidance = rule
            .get("guidance")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = rule
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("runtime-rule");
        let version = rule.get("version").and_then(Value::as_u64).unwrap_or(1);
        if !guidance.is_empty() && guidance.len() <= MAX_GUIDANCE_BYTES {
            response["surfari_runtime_learning"] = json!({
                "rule_id": id,
                "rule_version": version,
                "match_method": method,
                "score_micros": score,
                "url_base": domain,
                "tags": observed_tags,
                "guidance": guidance,
            });
            if let Some(object) = response.as_object_mut() {
                object.insert(
                    "error".to_string(),
                    Value::String(format!(
                        "{error}\nSurfari runtime guidance [{id}@{version}]: {guidance}"
                    )),
                );
            }
        }
        (Some(id.to_string()), Some(version), Some(method))
    } else {
        (None, None, None)
    };

    append_receipt(&json!({
        "event_type": "runtime_learning_evaluated",
        "schema_version": 1,
        "timestamp": timestamp(),
        "browser_session": browser_session,
        "use_id": use_id,
        "url_base": domain,
        "observed_tags": observed_tags,
        "error_digest": error_digest,
        "disposition": disposition,
        "rule_id": rule_id,
        "rule_version": rule_version,
        "match_method": match_method,
    }));
}

fn load_rules() -> Option<Value> {
    let path = rules_path()?;
    let metadata = fs::metadata(&path).ok()?;
    if metadata.len() > MAX_RULE_BYTES {
        return Some(json!({"schema_version": 0}));
    }
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn select_rule(
    rules: &[Value],
    domain: Option<&str>,
    observed_tags: &BTreeSet<String>,
) -> Option<(Value, &'static str, u64)> {
    let approved: Vec<&Value> = rules
        .iter()
        .take(MAX_SEMANTIC_RULES)
        .filter(|rule| rule.get("status").and_then(Value::as_str) == Some("approved"))
        .collect();

    if let Some(domain) = domain {
        if let Some(rule) = approved.iter().find(|rule| domain_match(rule, domain)) {
            return Some(((*rule).clone(), "url_base", 1_000_000));
        }
    }

    if let Some(rule) = approved.iter().find(|rule| tags_match(rule, observed_tags)) {
        return Some(((*rule).clone(), "tags", 900_000));
    }

    approved
        .into_iter()
        .filter_map(|rule| {
            let terms = string_set(rule.get("semantic_terms"));
            let score = jaccard_micros(&terms, observed_tags);
            (score >= 250_000).then(|| (rule.clone(), "semantic_tokens", score))
        })
        .max_by(|left, right| {
            left.2.cmp(&right.2).then_with(|| {
                right
                    .0
                    .get("id")
                    .and_then(Value::as_str)
                    .cmp(&left.0.get("id").and_then(Value::as_str))
            })
        })
}

fn domain_match(rule: &Value, domain: &str) -> bool {
    rule.get("url_bases")
        .and_then(Value::as_array)
        .is_some_and(|bases| {
            bases.iter().filter_map(Value::as_str).any(|base| {
                let base = base.trim().trim_start_matches("*.");
                !base.is_empty() && (domain == base || domain.ends_with(&format!(".{base}")))
            })
        })
}

fn tags_match(rule: &Value, observed: &BTreeSet<String>) -> bool {
    let required = string_set(rule.get("tags"));
    !required.is_empty() && required.is_subset(observed)
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(normalize_token)
        .filter(|value| !value.is_empty())
        .collect()
}

fn classify(error: &str) -> BTreeSet<String> {
    let lower = error.to_ascii_lowercase();
    let mut tags = BTreeSet::new();
    for (needle, tag) in [
        ("auto-launch failed", "launch-failure"),
        ("no usable sandbox", "local-chrome-sandbox"),
        ("devtoolsactiveport", "chrome-startup"),
        ("provider", "provider"),
        ("browserbase", "browserbase"),
        ("credential", "credential-boundary"),
        ("timeout", "timeout"),
        ("session", "session"),
    ] {
        if lower.contains(needle) {
            tags.insert(tag.to_string());
        }
    }
    tags
}

fn normalize_token(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn jaccard_micros(left: &BTreeSet<String>, right: &BTreeSet<String>) -> u64 {
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    let intersection = left.intersection(right).count() as u64;
    let union = left.union(right).count() as u64;
    intersection.saturating_mul(1_000_000) / union.max(1)
}

fn digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn append_receipt(event: &Value) {
    let Some(path) = events_path() else { return };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        #[cfg(unix)]
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let Ok(mut file) = options.open(path) else {
        return;
    };
    let _ = serde_json::to_writer(&mut file, event);
    let _ = file.write_all(b"\n");
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

    fn install_store(temp: &tempfile::TempDir, rules: Value) -> EnvGuard<'_> {
        let guard = EnvGuard::new(&[RULES_PATH_ENV, EVENTS_PATH_ENV]);
        let rules_path = temp.path().join("rules.json");
        fs::write(&rules_path, serde_json::to_vec(&rules).unwrap()).unwrap();
        guard.set(RULES_PATH_ENV, rules_path.to_str().unwrap());
        guard.set(
            EVENTS_PATH_ENV,
            temp.path().join("events.jsonl").to_str().unwrap(),
        );
        guard
    }

    fn browserbase_rule(guidance: &str) -> Value {
        json!({"schema_version": 1, "rules": [{
            "id": "browserbase-transient-scope",
            "version": 1,
            "status": "approved",
            "url_bases": ["browserbase.com"],
            "tags": ["launch-failure", "local-chrome-sandbox"],
            "semantic_terms": ["launch-failure", "local-chrome-sandbox", "provider", "session"],
            "guidance": guidance
        }]})
    }

    #[test]
    fn exact_url_rule_reloads_without_compilation() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = install_store(&temp, browserbase_rule("relaunch through broker scope"));
        let mut response =
            json!({"success": false, "error": "Auto-launch failed: No usable sandbox"});
        apply(&mut response, Some("www.browserbase.com"), "admin", None);
        assert_eq!(
            response["surfari_runtime_learning"]["match_method"],
            "url_base"
        );
        assert!(response["error"]
            .as_str()
            .unwrap()
            .contains("relaunch through broker scope"));

        fs::write(
            temp.path().join("rules.json"),
            serde_json::to_vec(&browserbase_rule("new recovery")).unwrap(),
        )
        .unwrap();
        let mut reloaded =
            json!({"success": false, "error": "Auto-launch failed: No usable sandbox"});
        apply(&mut reloaded, Some("browserbase.com"), "admin", None);
        assert!(reloaded["error"].as_str().unwrap().contains("new recovery"));
    }

    #[test]
    fn tags_then_semantic_fallback_are_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = install_store(&temp, browserbase_rule("recover"));
        let mut tagged =
            json!({"success": false, "error": "Auto-launch failed: No usable sandbox"});
        apply(&mut tagged, None, "admin", None);
        assert_eq!(tagged["surfari_runtime_learning"]["match_method"], "tags");

        let mut semantic = json!({"success": false, "error": "provider session launch-failure"});
        apply(&mut semantic, None, "admin", None);
        assert_eq!(
            semantic["surfari_runtime_learning"]["match_method"],
            "semantic_tokens"
        );
    }

    #[test]
    fn only_approved_and_matching_rules_are_retrieved() {
        let temp = tempfile::tempdir().unwrap();
        let mut rules = browserbase_rule("recover");
        rules["rules"][0]["status"] = Value::String("proposed".to_string());
        let _guard = install_store(&temp, rules);
        let mut response =
            json!({"success": false, "error": "Auto-launch failed: No usable sandbox"});
        apply(&mut response, Some("browserbase.com"), "admin", None);
        assert!(response.get("surfari_runtime_learning").is_none());
    }

    #[test]
    fn logs_redacted_receipt_not_raw_error() {
        let temp = tempfile::tempdir().unwrap();
        let _guard = install_store(&temp, browserbase_rule("recover"));
        let secret = "password=hunter2 cookie=session-secret";
        let mut response = json!({"success": false, "error": secret});
        apply(&mut response, Some("example.com"), "admin", Some("use-1"));
        let events = fs::read_to_string(temp.path().join("events.jsonl")).unwrap();
        assert!(!events.contains("hunter2"));
        assert!(!events.contains("session-secret"));
        assert!(events.contains("error_digest"));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(temp.path().join("events.jsonl"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn corrupt_or_oversized_store_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let guard = EnvGuard::new(&[RULES_PATH_ENV, EVENTS_PATH_ENV]);
        let path = temp.path().join("rules.json");
        fs::write(&path, "{broken").unwrap();
        guard.set(RULES_PATH_ENV, path.to_str().unwrap());
        guard.set(
            EVENTS_PATH_ENV,
            temp.path().join("events.jsonl").to_str().unwrap(),
        );
        let mut response = json!({"success": false, "error": "failed"});
        apply(&mut response, Some("browserbase.com"), "admin", None);
        assert!(response.get("surfari_runtime_learning").is_none());
        fs::write(&path, vec![b'x'; MAX_RULE_BYTES as usize + 1]).unwrap();
        apply(&mut response, Some("browserbase.com"), "admin", None);
        assert!(response.get("surfari_runtime_learning").is_none());
    }

    #[test]
    fn latest_session_domain_uses_redacted_history() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("events.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"browser_session\":\"other\",\"learning_context\":{\"domain\":\"example.com\"}}\n",
                "{\"browser_session\":\"admin\",\"learning_context\":{\"domain\":\"browserbase.com\"}}\n"
            ),
        )
        .unwrap();
        assert_eq!(
            latest_domain(&path, "admin").as_deref(),
            Some("browserbase.com")
        );
    }
}
