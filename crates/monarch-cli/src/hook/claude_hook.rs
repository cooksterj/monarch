//! A Claude Code hook event as an `agent.report`.
//!
//! Event names and fields follow the Claude Code hooks reference:
//! `hook_event_name`, `session_id` and `cwd` on every event, plus
//! `prompt`, `tool_name`/`tool_input`, `notification_type`/`message` and
//! `last_assistant_message` on the events that carry them.

use monarch_core::{AgentKind, AgentState, PaneId};
use monarch_proto::AgentReport;
use serde_json::Value;

use super::tool_activity::ToolActivity;

/// One hook call's JSON input.
pub(crate) struct ClaudeHook<'a> {
    input: &'a Value,
}

impl<'a> ClaudeHook<'a> {
    /// Wraps the JSON a hook received on stdin.
    pub fn new(input: &'a Value) -> Self {
        Self { input }
    }

    /// What to tell the server about `pane`, or `None` for events that
    /// change nothing (an idle reminder, a subagent finishing).
    pub fn report(&self, pane: PaneId) -> Option<AgentReport> {
        let (state, activity) = self.meaning()?;
        Some(AgentReport {
            state: Some(state),
            activity,
            session_id: self.field("session_id").map(str::to_owned),
            ..AgentReport::new(pane, AgentKind::Claude)
        })
    }

    /// The event's state and new activity line (`""` clears it).
    fn meaning(&self) -> Option<(AgentState, Option<String>)> {
        let text = |key: &str| self.field(key).map(str::to_owned);
        Some(match self.field("hook_event_name")? {
            "SessionStart" => (AgentState::Idle, Some(String::new())),
            "UserPromptSubmit" => (AgentState::Working, text("prompt")),
            "PreToolUse" => (AgentState::Working, Some(self.tool().line())),
            "PostToolUse" | "PostToolUseFailure" => (AgentState::Working, None),
            "PreCompact" => (AgentState::Working, Some("Compacting".into())),
            "PermissionRequest" => (
                AgentState::Blocked,
                Some(format!("Allow {}?", self.tool().line())),
            ),
            "Notification" if self.needs_input() => (AgentState::Blocked, text("message")),
            "Stop" => (AgentState::Done, text("last_assistant_message")),
            "StopFailure" => (AgentState::Error, text("error").or_else(|| text("message"))),
            "SessionEnd" => (AgentState::Unknown, Some(String::new())),
            _ => return None,
        })
    }

    /// A notification that waits on the user (a permission prompt or a
    /// question), not the reminder that Claude has been idle a while.
    fn needs_input(&self) -> bool {
        match self.field("notification_type") {
            Some(kind) => kind != "idle_prompt" && kind != "auth_success",
            None => self
                .field("message")
                .is_some_and(|m| m.to_ascii_lowercase().contains("permission")),
        }
    }

    fn tool(&self) -> ToolActivity<'a> {
        ToolActivity {
            tool: self.field("tool_name").unwrap_or("tool"),
            input: self.input.get("tool_input"),
            cwd: self.field("cwd"),
        }
    }

    fn field(&self, key: &str) -> Option<&'a str> {
        self.input.get(key)?.as_str()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn report(input: &Value) -> Option<(Option<AgentState>, Option<String>)> {
        let report = ClaudeHook::new(input).report(PaneId(4))?;
        assert_eq!((report.pane, report.agent), (PaneId(4), AgentKind::Claude));
        Some((report.state, report.activity))
    }

    fn says(state: AgentState, activity: &str) -> Option<(Option<AgentState>, Option<String>)> {
        Some((Some(state), Some(activity.into())))
    }

    #[test]
    fn a_turn_goes_working_then_done() {
        let prompt = json!({"hook_event_name": "UserPromptSubmit", "prompt": "fix the bug"});
        assert_eq!(report(&prompt), says(AgentState::Working, "fix the bug"));
        let tool = json!({
            "hook_event_name": "PreToolUse", "cwd": "/w",
            "tool_name": "Edit", "tool_input": {"file_path": "/w/src/a.rs"}
        });
        assert_eq!(report(&tool), says(AgentState::Working, "Edit src/a.rs"));
        let post = json!({"hook_event_name": "PostToolUse", "tool_name": "Edit"});
        assert_eq!(report(&post), Some((Some(AgentState::Working), None)));
        let stop = json!({"hook_event_name": "Stop", "last_assistant_message": "Fixed it."});
        assert_eq!(report(&stop), says(AgentState::Done, "Fixed it."));
    }

    #[test]
    fn prompts_for_the_user_block() {
        let ask = json!({
            "hook_event_name": "Notification", "notification_type": "permission_prompt",
            "message": "Claude needs your permission to use Bash"
        });
        assert_eq!(
            report(&ask),
            says(
                AgentState::Blocked,
                "Claude needs your permission to use Bash"
            )
        );
        let old = json!({"hook_event_name": "Notification", "message": "Needs permission"});
        assert_eq!(report(&old).and_then(|r| r.0), Some(AgentState::Blocked));
        let request = json!({
            "hook_event_name": "PermissionRequest", "tool_name": "Bash",
            "tool_input": {"command": "rm -rf target"}
        });
        assert_eq!(
            report(&request),
            says(AgentState::Blocked, "Allow Bash rm -rf target?")
        );
    }

    #[test]
    fn reminders_and_unknown_events_change_nothing() {
        let idle = json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"});
        assert_eq!(report(&idle), None);
        assert_eq!(report(&json!({"hook_event_name": "SubagentStop"})), None);
        assert_eq!(report(&json!({"no": "event"})), None);
    }

    #[test]
    fn sessions_start_idle_and_end_as_a_shell() {
        let start = json!({"hook_event_name": "SessionStart", "session_id": "abc"});
        let sent = ClaudeHook::new(&start).report(PaneId(1)).expect("report");
        assert_eq!(sent.session_id.as_deref(), Some("abc"));
        assert_eq!(report(&start), says(AgentState::Idle, ""));
        let end = json!({"hook_event_name": "SessionEnd"});
        assert_eq!(report(&end), says(AgentState::Unknown, ""));
        let failed = json!({"hook_event_name": "StopFailure", "error": "rate limited"});
        assert_eq!(report(&failed), says(AgentState::Error, "rate limited"));
    }
}
