use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;

use super::super::actions::DaemonState;
use super::super::browser::format_tab_id;

pub fn capture(state: &DaemonState) -> Value {
    let page = state
        .browser
        .as_ref()
        .and_then(|browser| browser.active_page_info());
    let tab_count = state
        .browser
        .as_ref()
        .map(|browser| browser.page_count())
        .unwrap_or(0);
    let context_id = std::env::var("SURFARI_CONTEXT_ID")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    json!({
        "context_id": context_id,
        "surfari_session_id": state.session_id,
        "browser_session": state.session_name.as_deref().unwrap_or(&state.session_id),
        "engine": state.engine,
        "tab_count": tab_count,
        "active_tab_id": page.as_ref().map(|p| format_tab_id(p.tab_id)),
        "active_tab_label": page.as_ref().and_then(|p| p.label.clone()),
        "active_target_id": page.as_ref().map(|p| p.target_id.clone()),
        "active_cdp_session_id_hash": page.as_ref().map(|p| hash_string(&p.session_id)),
        "active_url": page.as_ref().and_then(|p| sanitize_url(&p.url)),
        "active_title": page.as_ref().and_then(|p| summarize_title(&p.title)),
    })
}

fn sanitize_url(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let mut parsed = Url::parse(raw).ok()?;
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

fn summarize_title(title: &str) -> Option<Value> {
    if title.is_empty() {
        return None;
    }
    Some(json!({
        "byte_length": title.len(),
        "sha256": hash_string(title),
    }))
}

fn hash_string(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::browser::PageInfo;

    #[test]
    fn sanitizes_url_query_and_fragment() {
        assert_eq!(
            sanitize_url("https://example.com/path?token=secret#frag"),
            Some("https://example.com/path".to_string())
        );
    }

    #[test]
    fn title_summary_does_not_include_raw_title() {
        let summary = summarize_title("Secret dashboard title").unwrap();
        let text = serde_json::to_string(&summary).unwrap();

        assert!(!text.contains("Secret dashboard title"));
        assert_eq!(summary["byte_length"], 22);
    }

    #[test]
    fn no_browser_anchor_still_records_session() {
        let mut state = DaemonState::new();
        state.session_id = "anchor-test-session".to_string();
        let anchor = capture(&state);

        assert_eq!(anchor["surfari_session_id"], "anchor-test-session");
        assert_eq!(anchor["tab_count"], 0);
        assert_eq!(anchor["active_tab_id"], Value::Null);
    }

    #[test]
    fn page_anchor_uses_stable_tab_id_and_safe_metadata() {
        let page = PageInfo {
            tab_id: 2,
            label: Some("gmail".to_string()),
            target_id: "target-123".to_string(),
            session_id: "cdp-session-secret".to_string(),
            url: "https://mail.google.com/mail/u/0/?authuser=daniel#inbox".to_string(),
            title: "Daniel private inbox".to_string(),
            target_type: "page".to_string(),
        };

        let anchor = json!({
            "active_tab_id": format_tab_id(page.tab_id),
            "active_tab_label": page.label,
            "active_target_id": page.target_id,
            "active_cdp_session_id_hash": hash_string(&page.session_id),
            "active_url": sanitize_url(&page.url),
            "active_title": summarize_title(&page.title),
        });
        let text = serde_json::to_string(&anchor).unwrap();

        assert_eq!(anchor["active_tab_id"], "t2");
        assert_eq!(anchor["active_tab_label"], "gmail");
        assert_eq!(anchor["active_url"], "https://mail.google.com/mail/u/0/");
        assert!(!text.contains("cdp-session-secret"));
        assert!(!text.contains("Daniel private inbox"));
        assert!(!text.contains("authuser=daniel"));
    }
}
