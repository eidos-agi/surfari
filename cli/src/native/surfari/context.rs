use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use url::Url;

pub const CONTEXT_ID_ENV: &str = "SURFARI_CONTEXT_ID";
pub const ORG_ID_ENV: &str = "SURFARI_ORG_ID";
pub const ACCOUNT_ID_ENV: &str = "SURFARI_ACCOUNT_ID";
pub const PROFILE_ID_ENV: &str = "SURFARI_PROFILE_ID";
pub const SUBJECT_ID_ENV: &str = "SURFARI_SUBJECT_ID";
pub const KNOX_REF_ENV: &str = "SURFARI_KNOX_REF";
pub const EXPECTED_DOMAINS_ENV: &str = "SURFARI_EXPECTED_DOMAINS";
pub const BROWSER_PROFILE_PATH_ENV: &str = "SURFARI_BROWSER_PROFILE_PATH";

pub fn capture() -> Value {
    let context_id = env_value(CONTEXT_ID_ENV);
    let org_id = env_value(ORG_ID_ENV);
    let account_id = env_value(ACCOUNT_ID_ENV);
    let profile_id = env_value(PROFILE_ID_ENV);
    let subject_id = env_value(SUBJECT_ID_ENV);
    let knox_ref = env_value(KNOX_REF_ENV);
    let expected_domains = expected_domains();
    let browser_profile_path = env_value(BROWSER_PROFILE_PATH_ENV)
        .as_deref()
        .map(summarize_value);
    let has_context_id = context_id.is_some();
    let has_context_detail = org_id.is_some()
        || account_id.is_some()
        || profile_id.is_some()
        || subject_id.is_some()
        || knox_ref.is_some()
        || !expected_domains.is_empty()
        || browser_profile_path.is_some();
    let status = context_status(has_context_id, has_context_detail);

    json!({
        "status": status,
        "context_id": context_id,
        "org_id": org_id,
        "account_id": account_id,
        "profile_id": profile_id,
        "subject_id": subject_id,
        "knox_ref": knox_ref,
        "expected_domains": expected_domains,
        "browser_profile_path": browser_profile_path,
    })
}

fn env_value(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn expected_domains() -> Vec<String> {
    env_value(EXPECTED_DOMAINS_ENV)
        .map(|raw| {
            raw.split(',')
                .filter_map(sanitize_domain)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

fn sanitize_domain(raw: &str) -> Option<String> {
    let trimmed = raw.trim().to_lowercase();
    if trimmed.is_empty() {
        return None;
    }

    let wildcard = trimmed.starts_with("*.");
    let without_wildcard = trimmed.strip_prefix("*.").unwrap_or(&trimmed);
    let host = if without_wildcard.contains("://") {
        Url::parse(without_wildcard)
            .ok()
            .and_then(|url| url.host_str().map(|host| host.to_string()))?
    } else {
        without_wildcard
            .split('/')
            .next()
            .unwrap_or_default()
            .split(':')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    let host = host.trim_matches('.');
    if !is_safe_domain(host) {
        return None;
    }
    if wildcard {
        Some(format!("*.{host}"))
    } else {
        Some(host.to_string())
    }
}

fn is_safe_domain(host: &str) -> bool {
    !host.is_empty()
        && host.contains('.')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
        && host.split('.').all(|label| !label.is_empty())
}

fn summarize_value(value: &str) -> Value {
    json!({
        "byte_length": value.len(),
        "sha256": hash_string(value),
    })
}

fn context_status(has_context_id: bool, has_context_detail: bool) -> &'static str {
    if has_context_id && has_context_detail {
        "set"
    } else if has_context_id || has_context_detail {
        "partial"
    } else {
        "unset"
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
    use crate::test_utils::EnvGuard;

    #[test]
    fn capture_records_structured_context() {
        let guard = EnvGuard::new(&[
            CONTEXT_ID_ENV,
            ORG_ID_ENV,
            ACCOUNT_ID_ENV,
            PROFILE_ID_ENV,
            SUBJECT_ID_ENV,
            KNOX_REF_ENV,
            EXPECTED_DOMAINS_ENV,
            BROWSER_PROFILE_PATH_ENV,
        ]);
        guard.set(CONTEXT_ID_ENV, "eidos");
        guard.set(ORG_ID_ENV, "org-eidos");
        guard.set(ACCOUNT_ID_ENV, "acct-prod");
        guard.set(PROFILE_ID_ENV, "chrome-eidos");
        guard.set(SUBJECT_ID_ENV, "daniel-work");
        guard.set(KNOX_REF_ENV, "knox://surfari/eidos");
        guard.set(
            EXPECTED_DOMAINS_ENV,
            "https://linear.app/eidos-agi?token=secret, *.google.com, bad token",
        );
        guard.set(
            BROWSER_PROFILE_PATH_ENV,
            "/Users/dshanklin/Library/Chrome/Profile 1",
        );

        let context = capture();
        let text = serde_json::to_string(&context).unwrap();

        assert_eq!(context["status"], "set");
        assert_eq!(context["context_id"], "eidos");
        assert_eq!(context["org_id"], "org-eidos");
        assert_eq!(context["account_id"], "acct-prod");
        assert_eq!(context["profile_id"], "chrome-eidos");
        assert_eq!(context["subject_id"], "daniel-work");
        assert_eq!(context["knox_ref"], "knox://surfari/eidos");
        assert_eq!(context["expected_domains"][0], "linear.app");
        assert_eq!(context["expected_domains"][1], "*.google.com");
        assert!(!text.contains("token=secret"));
        assert!(!text.contains("/Users/dshanklin/Library/Chrome/Profile 1"));
        assert!(context["browser_profile_path"]["sha256"].is_string());
    }

    #[test]
    fn empty_context_is_unset() {
        let guard = EnvGuard::new(&[
            CONTEXT_ID_ENV,
            ORG_ID_ENV,
            ACCOUNT_ID_ENV,
            PROFILE_ID_ENV,
            SUBJECT_ID_ENV,
            KNOX_REF_ENV,
            EXPECTED_DOMAINS_ENV,
            BROWSER_PROFILE_PATH_ENV,
        ]);
        for name in [
            CONTEXT_ID_ENV,
            ORG_ID_ENV,
            ACCOUNT_ID_ENV,
            PROFILE_ID_ENV,
            SUBJECT_ID_ENV,
            KNOX_REF_ENV,
            EXPECTED_DOMAINS_ENV,
            BROWSER_PROFILE_PATH_ENV,
        ] {
            guard.remove(name);
        }

        let context = capture();

        assert_eq!(context["status"], "unset");
        assert_eq!(context["expected_domains"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn context_id_only_is_partial() {
        let guard = EnvGuard::new(&[
            CONTEXT_ID_ENV,
            ORG_ID_ENV,
            ACCOUNT_ID_ENV,
            PROFILE_ID_ENV,
            SUBJECT_ID_ENV,
            KNOX_REF_ENV,
            EXPECTED_DOMAINS_ENV,
            BROWSER_PROFILE_PATH_ENV,
        ]);
        guard.set(CONTEXT_ID_ENV, "eidos");
        for name in [
            ORG_ID_ENV,
            ACCOUNT_ID_ENV,
            PROFILE_ID_ENV,
            SUBJECT_ID_ENV,
            KNOX_REF_ENV,
            EXPECTED_DOMAINS_ENV,
            BROWSER_PROFILE_PATH_ENV,
        ] {
            guard.remove(name);
        }

        let context = capture();

        assert_eq!(context["status"], "partial");
        assert_eq!(context["context_id"], "eidos");
    }
}
