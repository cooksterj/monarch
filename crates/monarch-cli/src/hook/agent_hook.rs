//! An agent's hook event as an `agent.report`.
//!
//! Claude Code and Codex share one hook format: `hook_event_name`,
//! `session_id` and `cwd` on every event, plus `prompt`,
//! `tool_name`/`tool_input` and `last_assistant_message` on the events
//! that carry them. Claude adds `Notification` (`notification_type`,
//! `message`); Codex adds `Interrupt`. Events one agent never sends cost
//! nothing, so a single mapping serves both.

use monarch_core::{AgentKind, AgentState, PaneId};
use monarch_proto::AgentReport;
use serde_json::Value;

use super::tool_activity::ToolActivity;

/// One hook call's JSON input, from `agent`.
pub(crate) struct AgentHook<'a> {
    agent: AgentKind,
    input: &'a Value,
}

impl<'a> AgentHook<'a> {
    /// Wraps the JSON `agent`'s hook received on stdin.
    pub fn new(agent: AgentKind, input: &'a Value) -> Self {
        Self { agent, input }
    }

    /// What to tell the server about `pane` (`None` when the hook can't
    /// know it), or `None` for events that change nothing (an idle
    /// reminder, a subagent finishing).
    pub fn report(&self, pane: Option<PaneId>) -> Option<AgentReport> {
        let (state, activity) = self.meaning()?;
        Some(AgentReport {
            pane,
            state: Some(state),
            activity,
            session_id: self.field("session_id").map(str::to_owned),
            cwd: self.field("cwd").map(str::to_owned),
            ..AgentReport::unplaced(self.agent)
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
            "Interrupt" => (AgentState::Idle, Some("Interrupted".into())),
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
        let report = AgentHook::new(AgentKind::Claude, input).report(Some(PaneId(4)))?;
        assert_eq!(
            (report.pane, report.agent),
            (Some(PaneId(4)), AgentKind::Claude)
        );
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
        let sent = AgentHook::new(AgentKind::Claude, &start)
            .report(Some(PaneId(1)))
            .expect("report");
        assert_eq!(sent.session_id.as_deref(), Some("abc"));
        assert_eq!(report(&start), says(AgentState::Idle, ""));
        let end = json!({"hook_event_name": "SessionEnd"});
        assert_eq!(report(&end), says(AgentState::Unknown, ""));
        let failed = json!({"hook_event_name": "StopFailure", "error": "rate limited"});
        assert_eq!(report(&failed), says(AgentState::Error, "rate limited"));
    }

    #[test]
    fn codex_events_report_as_codex() {
        let patch = json!({
            "hook_event_name": "PreToolUse", "cwd": "/w", "tool_name": "apply_patch",
            "tool_input": {"command": "*** Begin Patch\n*** Update File: /w/src/lib.rs\n@@\n-a\n+b\n"}
        });
        let sent = AgentHook::new(AgentKind::Codex, &patch)
            .report(None)
            .expect("report");
        assert_eq!(sent.cwd.as_deref(), Some("/w"));
        assert_eq!(sent.agent, AgentKind::Codex);
        assert_eq!(sent.activity.as_deref(), Some("Edit src/lib.rs"));
        let interrupt = json!({"hook_event_name": "Interrupt", "turn_id": "t1"});
        assert_eq!(report(&interrupt), says(AgentState::Idle, "Interrupted"));
    }
}
