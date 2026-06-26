pub mod action_log;
pub mod browser_anchor;
pub mod context;
pub mod governance;
pub mod learning;
pub mod redaction;

use serde_json::{json, Value};
use std::env;
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

use super::actions::DaemonState;

const HUMAN_GATE_DOMAINS: &[&str] = &["idmsa.apple.com", "appleid.apple.com"];

pub fn status() -> Value {
    let use_id = action_log::current_use_id();
    let action_log_path = action_log::resolve_log_path(use_id.as_deref());
    let learning_candidates_path = action_log::resolve_learning_candidates_path(use_id.as_deref());
    let executable_path = env::current_exe()
        .ok()
        .map(|path| path.display().to_string());

    json!({
        "product": "Surfari",
        "binary": "agent-browser",
        "version": env!("CARGO_PKG_VERSION"),
        "governance": {
            "status": "available",
            "context": context::capture(),
            "human_gate_domains": HUMAN_GATE_DOMAINS,
            "protected_action_policy": "fail_closed_when_context_or_expected_domain_mismatch",
        },
        "logging": {
            "use_id": use_id,
            "action_log_path": action_log_path.display().to_string(),
            "learning_candidates_path": learning_candidates_path
                .map(|path| path.display().to_string()),
        },
        "install": {
            "executable_path": executable_path,
            "source": "native_fork_binary",
            "wrapper_required": false,
        },
        "boundaries": {
            "apple_login": "human_gate",
            "mfa": "human_gate",
            "passkeys": "human_gate",
            "legal_agreements": "human_gate",
            "payments": "human_gate",
            "profile_download_install": "human_gate",
            "final_submission": "human_gate",
        }
    })
}

pub struct ActionLogScope {
    action_id: String,
    command_id: String,
    action: String,
    browser_session: String,
    use_id: Option<String>,
    cwd: String,
    started_at: Instant,
    log_path: Option<PathBuf>,
    candidate_path: Option<PathBuf>,
    learning_context: Value,
    command_metadata: Value,
    surfari_context: Value,
    browser_anchor_before: Value,
    governance_decision: governance::Decision,
}

impl ActionLogScope {
    pub fn start(cmd: &Value, state: &DaemonState) -> Self {
        let action = cmd
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let command_id = cmd
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let use_id = current_use_id(state);
        let browser_session = state
            .session_name
            .clone()
            .unwrap_or_else(|| state.session_id.clone());
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let log_path = resolve_log_path(state, use_id.as_deref());
        let candidate_path = resolve_candidate_path(state, use_id.as_deref());
        let learning_context = learning::load_context(use_id.as_deref(), &browser_session, cmd);
        let command_metadata = learning::command_metadata(cmd);
        let surfari_context = context::capture();
        let browser_anchor_before = browser_anchor::capture(state);
        let governance_decision =
            governance::evaluate(cmd, &surfari_context, &browser_anchor_before);
        let scope = Self {
            action_id: Uuid::new_v4().to_string(),
            command_id,
            action,
            browser_session,
            use_id,
            cwd,
            started_at: Instant::now(),
            log_path,
            candidate_path,
            learning_context,
            command_metadata,
            surfari_context,
            browser_anchor_before,
            governance_decision,
        };
        let event = json!({
            "event_type": "action_started",
            "action_id": scope.action_id,
            "command_id": scope.command_id,
            "action": scope.action,
            "browser_session": scope.browser_session,
            "use_id": scope.use_id,
            "cwd": scope.cwd,
            "safety_class": redaction::safety_class(&scope.action),
            "redaction_status": redaction::redaction_status(cmd),
            "command": redaction::redact_command(cmd),
            "learning_context": scope.learning_context.clone(),
            "surfari_context": scope.surfari_context.clone(),
            "browser_anchor": scope.browser_anchor_before.clone(),
            "governance": scope.governance_decision.metadata.clone(),
        });
        if let Some(log_path) = &scope.log_path {
            let _ = action_log::append_event(log_path, &event);
        }
        scope
    }

    pub fn governance_error(&self) -> Option<String> {
        self.governance_decision.blocked.then(|| {
            format!(
                "Surfari context governance blocked protected action '{}': {}",
                self.action, self.governance_decision.reason
            )
        })
    }

    pub fn finish(&self, response: &Value, state: &DaemonState) {
        let Some(log_path) = &self.log_path else {
            return;
        };
        let duration_ms = self.started_at.elapsed().as_millis() as u64;
        let browser_anchor_after = browser_anchor::capture(state);
        let event = json!({
            "event_type": "action_finished",
            "action_id": self.action_id,
            "command_id": self.command_id,
            "action": self.action,
            "browser_session": self.browser_session,
            "use_id": self.use_id,
            "cwd": self.cwd,
            "duration_ms": duration_ms,
            "success": response.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
            "result": redaction::summarize_response(response),
            "surfari_context": self.surfari_context.clone(),
            "browser_anchor": browser_anchor_after.clone(),
            "governance": self.governance_decision.metadata.clone(),
        });
        let _ = action_log::append_event(log_path, &event);
        if let Some(candidate_path) = &self.candidate_path {
            let candidate = learning::candidate_from_response(learning::CandidateInput {
                action_id: &self.action_id,
                command_id: &self.command_id,
                action: &self.action,
                browser_session: &self.browser_session,
                use_id: self.use_id.as_deref(),
                learning_context: &self.learning_context,
                command_metadata: &self.command_metadata,
                surfari_context: &self.surfari_context,
                browser_anchor: &browser_anchor_after,
                response,
            });
            let _ = action_log::append_event(candidate_path, &candidate);
        }
    }
}

#[cfg(not(test))]
fn current_use_id(_state: &DaemonState) -> Option<String> {
    action_log::current_use_id()
}

#[cfg(test)]
fn current_use_id(state: &DaemonState) -> Option<String> {
    state.test_surfari_use_id.clone()
}

#[cfg(not(test))]
fn resolve_log_path(_state: &DaemonState, use_id: Option<&str>) -> Option<PathBuf> {
    Some(action_log::resolve_log_path(use_id))
}

#[cfg(test)]
fn resolve_log_path(state: &DaemonState, _use_id: Option<&str>) -> Option<PathBuf> {
    state.test_surfari_action_log_path.clone()
}

#[cfg(not(test))]
fn resolve_candidate_path(_state: &DaemonState, use_id: Option<&str>) -> Option<PathBuf> {
    action_log::resolve_learning_candidates_path(use_id)
}

#[cfg(test)]
fn resolve_candidate_path(state: &DaemonState, _use_id: Option<&str>) -> Option<PathBuf> {
    state.test_surfari_learning_candidates_path.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvGuard;

    #[test]
    fn status_reports_native_surfari_without_wrapper() {
        let guard = EnvGuard::new(&[
            context::CONTEXT_ID_ENV,
            context::ORG_ID_ENV,
            context::ACCOUNT_ID_ENV,
            context::PROFILE_ID_ENV,
            context::SUBJECT_ID_ENV,
            context::KNOX_REF_ENV,
            context::EXPECTED_DOMAINS_ENV,
            context::BROWSER_PROFILE_PATH_ENV,
            action_log::ACTION_LOG_PATH_ENV,
            action_log::USE_ID_ENV,
        ]);
        guard.set(context::CONTEXT_ID_ENV, "eid-448");
        guard.set(
            context::EXPECTED_DOMAINS_ENV,
            "developer.apple.com,idmsa.apple.com",
        );
        guard.set(action_log::USE_ID_ENV, "native-status-test");

        let status = status();

        assert_eq!(status["product"], "Surfari");
        assert_eq!(status["install"]["source"], "native_fork_binary");
        assert_eq!(status["install"]["wrapper_required"], false);
        assert_eq!(status["governance"]["status"], "available");
        assert_eq!(
            status["governance"]["context"]["expected_domains"][0],
            "developer.apple.com"
        );
        assert_eq!(status["boundaries"]["mfa"], "human_gate");
        assert!(status["logging"]["action_log_path"]
            .as_str()
            .unwrap()
            .ends_with("native-status-test/browser-actions.jsonl"));
    }
}
