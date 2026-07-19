pub mod action_log;
pub mod browser_anchor;
pub mod context;
pub mod governance;
pub mod learning;
pub mod redaction;
pub mod runtime_learning;

use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

use super::actions::DaemonState;

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
        let mut learning_context = learning::load_context(use_id.as_deref(), &browser_session, cmd);
        if learning_context.get("domain").is_none_or(Value::is_null) {
            if let Some(domain) = log_path
                .as_deref()
                .and_then(|path| runtime_learning::latest_domain(path, &browser_session))
            {
                learning_context["domain"] = Value::String(domain);
            }
        }
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

    pub fn apply_runtime_learning(&self, response: &mut Value) {
        runtime_learning::apply(
            response,
            self.learning_context.get("domain").and_then(Value::as_str),
            &self.browser_session,
            self.use_id.as_deref(),
        );
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
