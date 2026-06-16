use serde_json::{json, Value};
use url::Url;

#[derive(Clone, Debug)]
pub struct Decision {
    pub blocked: bool,
    pub reason: &'static str,
    pub metadata: Value,
}

pub fn evaluate(cmd: &Value, surfari_context: &Value, browser_anchor: &Value) -> Decision {
    let action = cmd.get("action").and_then(|v| v.as_str()).unwrap_or("");
    let protected = is_protected_action(action);
    let expected_domains = expected_domains(surfari_context);
    let observed_domain = domain_from_command(cmd).or_else(|| domain_from_anchor(browser_anchor));
    let human_gate = observed_domain.as_deref().and_then(human_gate_for_domain);
    let context_status = surfari_context
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unset");
    let governance_active = context_status != "unset" || !expected_domains.is_empty();

    let (blocked, reason) = if !governance_active {
        (false, "governance_inactive")
    } else if !protected {
        (false, "read_only_action")
    } else if context_status == "unset" {
        (true, "no_context")
    } else if context_status == "partial"
        || expected_domains.is_empty()
        || observed_domain.is_none()
    {
        (true, "partial_context")
    } else if !domain_matches_any(
        observed_domain.as_deref().unwrap_or_default(),
        &expected_domains,
    ) {
        (true, "domain_mismatch")
    } else if human_gate.is_some() {
        (true, "human_gate_required")
    } else {
        (false, "protected_action_allowed")
    };

    let decision = if blocked { "blocked" } else { "allowed" };
    Decision {
        blocked,
        reason,
        metadata: json!({
            "decision": decision,
            "reason": reason,
            "protected_action": protected,
            "context_status": context_status,
            "expected_domains": expected_domains,
            "observed_domain": observed_domain,
            "human_gate": human_gate,
        }),
    }
}

fn is_protected_action(action: &str) -> bool {
    matches!(
        action,
        "addscript"
            | "addinitscript"
            | "addstyle"
            | "auth_login"
            | "auth_save"
            | "check"
            | "clear"
            | "click"
            | "clipboard"
            | "cookies_clear"
            | "cookies_set"
            | "credentials"
            | "credentials_set"
            | "dblclick"
            | "dispatch"
            | "download"
            | "drag"
            | "fill"
            | "focus"
            | "geolocation"
            | "headers"
            | "input_keyboard"
            | "input_mouse"
            | "input_touch"
            | "inserttext"
            | "keydown"
            | "keyboard"
            | "mousedown"
            | "mousemove"
            | "mouseup"
            | "mouse"
            | "permissions"
            | "press"
            | "pushstate"
            | "removeinitscript"
            | "route"
            | "select"
            | "selectall"
            | "setcontent"
            | "setvalue"
            | "storage_clear"
            | "storage_set"
            | "swipe"
            | "tap"
            | "type"
            | "uncheck"
            | "unroute"
            | "upload"
    )
}

fn expected_domains(surfari_context: &Value) -> Vec<String> {
    surfari_context
        .get("expected_domains")
        .and_then(|v| v.as_array())
        .map(|domains| {
            domains
                .iter()
                .filter_map(|domain| domain.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn domain_from_command(cmd: &Value) -> Option<String> {
    cmd.get("url")
        .or_else(|| cmd.get("href"))
        .and_then(|v| v.as_str())
        .and_then(domain_from_url)
}

fn domain_from_anchor(browser_anchor: &Value) -> Option<String> {
    browser_anchor
        .get("active_url")
        .and_then(|v| v.as_str())
        .and_then(domain_from_url)
}

fn domain_from_url(raw: &str) -> Option<String> {
    Url::parse(raw)
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
}

fn domain_matches_any(observed: &str, expected_domains: &[String]) -> bool {
    let observed = observed.trim().trim_matches('.').to_ascii_lowercase();
    expected_domains
        .iter()
        .any(|expected| domain_matches(&observed, expected))
}

fn domain_matches(observed: &str, expected: &str) -> bool {
    let expected = expected.trim().trim_matches('.').to_ascii_lowercase();
    if let Some(base) = expected.strip_prefix("*.") {
        observed == base || observed.ends_with(&format!(".{base}"))
    } else {
        observed == expected
    }
}

fn human_gate_for_domain(domain: &str) -> Option<&'static str> {
    match domain
        .trim()
        .trim_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "idmsa.apple.com" | "appleid.apple.com" => Some("apple_sign_in"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_read_only_action_when_context_is_unset() {
        let decision = evaluate(
            &json!({"action": "snapshot", "url": "https://example.com"}),
            &json!({"status": "unset", "expected_domains": []}),
            &json!({}),
        );

        assert!(!decision.blocked);
        assert_eq!(decision.reason, "governance_inactive");
        assert_eq!(decision.metadata["protected_action"], false);
    }

    #[test]
    fn blocks_protected_action_when_context_is_partial() {
        let decision = evaluate(
            &json!({"action": "type", "url": "https://example.com/login"}),
            &json!({"status": "partial", "expected_domains": ["example.com"]}),
            &json!({}),
        );

        assert!(decision.blocked);
        assert_eq!(decision.reason, "partial_context");
        assert_eq!(decision.metadata["observed_domain"], "example.com");
    }

    #[test]
    fn blocks_protected_action_when_domain_mismatches() {
        let decision = evaluate(
            &json!({"action": "fill", "url": "https://evil.example/login"}),
            &json!({"status": "set", "expected_domains": ["example.com"]}),
            &json!({}),
        );

        assert!(decision.blocked);
        assert_eq!(decision.reason, "domain_mismatch");
        assert_eq!(decision.metadata["observed_domain"], "evil.example");
    }

    #[test]
    fn allows_protected_action_when_domain_matches() {
        let decision = evaluate(
            &json!({"action": "fill", "url": "https://mail.google.com/login"}),
            &json!({"status": "set", "expected_domains": ["*.google.com"]}),
            &json!({}),
        );

        assert!(!decision.blocked);
        assert_eq!(decision.reason, "protected_action_allowed");
    }

    #[test]
    fn uses_active_tab_domain_when_command_has_no_url() {
        let decision = evaluate(
            &json!({"action": "click", "selector": "#save"}),
            &json!({"status": "set", "expected_domains": ["linear.app"]}),
            &json!({"active_url": "https://linear.app/eidos-agi/issue/EID-397"}),
        );

        assert!(!decision.blocked);
        assert_eq!(decision.metadata["observed_domain"], "linear.app");
    }

    #[test]
    fn blocks_protected_action_on_apple_sign_in_domain() {
        let decision = evaluate(
            &json!({"action": "fill", "url": "https://idmsa.apple.com/appleauth/auth/signin"}),
            &json!({
                "status": "set",
                "expected_domains": ["developer.apple.com", "idmsa.apple.com"]
            }),
            &json!({}),
        );

        assert!(decision.blocked);
        assert_eq!(decision.reason, "human_gate_required");
        assert_eq!(decision.metadata["observed_domain"], "idmsa.apple.com");
        assert_eq!(decision.metadata["human_gate"], "apple_sign_in");
    }

    #[test]
    fn allows_read_only_action_on_apple_sign_in_domain() {
        let decision = evaluate(
            &json!({"action": "snapshot", "url": "https://idmsa.apple.com/appleauth/auth/signin"}),
            &json!({
                "status": "set",
                "expected_domains": ["developer.apple.com", "idmsa.apple.com"]
            }),
            &json!({}),
        );

        assert!(!decision.blocked);
        assert_eq!(decision.reason, "read_only_action");
        assert_eq!(decision.metadata["human_gate"], "apple_sign_in");
    }
}
