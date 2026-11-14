//! An agent's hook event as an `agent.report`.
//!
//! Claude Code and Codex share one hook format: `hook_event_name`,
//! `session_id` and `cwd` on every event, plus `prompt`,
//! `tool_name`/`tool_input` and `last_assistant_message` on the events
//! that carry them. Claude adds `Notification` (`notification_type`,
//! `message`); Codex adds `Interrupt`. Events one agent never sends cost
//! nothing, so a single mapping serves both.
//!
//! An event with an `agent_id` came from inside a sub-agent (Claude's
//! Agent tool, or a teammate): it reports that sub-agent and leaves the
//! parent's activity line alone, so parallel sub-agents don't fight over
//! it.

use monarch_core::{AgentKind, AgentState, PaneId};
use monarch_proto::{AgentReport, SubagentReport};
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
    /// reminder).
    pub fn report(&self, pane: Option<PaneId>) -> Option<AgentReport> {
        let base = AgentReport {
            pane,
            session_id: self.field("session_id").map(str::to_owned),
            cwd: self.field("cwd").map(str::to_owned),
            ..AgentReport::unplaced(self.agent)
        };
        if let Some(id) = self.field("agent_id").filter(|id| !id.is_empty()) {
            return self.subagent_report(id, base);
        }
        let (state, activity) = self.meaning()?;
        Some(AgentReport {
            state: Some(state),
            activity,
            subagents_running: self.subagents_running(),
            ..base
        })
    }

    /// An event inside sub-agent `id`: its own state and line, and the
    /// parent's state (working for it, or blocked on its question).
    fn subagent_report(&self, id: &str, base: AgentReport) -> Option<AgentReport> {
        let text = |key: &str| self.field(key).map(str::to_owned);
        let (parent, state, activity) = match self.field("hook_event_name")? {
            "SubagentStart" => (None, AgentState::Working, Some(String::new())),
            "PreToolUse" => (
                Some(AgentState::Working),
                AgentState::Working,
                Some(self.tool().line()),
            ),
            // After a question the row shows what ran, not the question.
            "PostToolUse" | "PostToolUseFailure" => (
                Some(AgentState::Working),
                AgentState::Working,
                Some(self.tool().line()),
            ),
            "PermissionRequest" => (
                Some(AgentState::Blocked),
                AgentState::Blocked,
                Some(format!("Allow {}?", self.tool().line())),
            ),
            "Notification" if self.needs_input() => (
                Some(AgentState::Blocked),
                AgentState::Blocked,
                text("message"),
            ),
            "SubagentStop" => (None, AgentState::Done, text("last_assistant_message")),
            _ => return None,
        };
        Some(AgentReport {
            state: parent,
            subagent: Some(SubagentReport {
                id: id.to_owned(),
                kind: text("agent_type"),
                state: Some(state),
                activity,
            }),
            ..base
        })
    }

    /// How many sub-agents still run in the background, from a `Stop`'s
    /// `background_tasks` (`None` when the event carries no list).
    fn subagents_running(&self) -> Option<u32> {
        let tasks = self.input.get("background_tasks")?.as_array()?;
        let running = tasks
            .iter()
            .filter(|t| t.get("type").and_then(Value::as_str) == Some("subagent"))
            .count();
        Some(u32::try_from(running).unwrap_or(u32::MAX))
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

    fn child(input: &Value) -> AgentReport {
        AgentHook::new(AgentKind::Claude, input)
            .report(Some(PaneId(4)))
            .expect("report")
    }

    fn sub(report: &AgentReport) -> (&str, Option<&str>, Option<AgentState>, Option<&str>) {
        let sub = report.subagent.as_ref().expect("a subagent report");
        (
            sub.id.as_str(),
            sub.kind.as_deref(),
            sub.state,
            sub.activity.as_deref(),
        )
    }

    #[test]
    fn subagent_events_report_the_subagent_not_the_parents_line() {
        let start = child(&json!({
            "hook_event_name": "SubagentStart", "agent_id": "a1", "agent_type": "Explore"
        }));
        assert_eq!(
            sub(&start),
            ("a1", Some("Explore"), Some(AgentState::Working), Some(""))
        );
        assert_eq!(
            (start.state, start.activity),
            (None, None),
            "parent untouched"
        );
        let tool = child(&json!({
            "hook_event_name": "PreToolUse", "agent_id": "a1", "agent_type": "Explore",
            "cwd": "/w", "tool_name": "Read", "tool_input": {"file_path": "/w/a.rs"}
        }));
        assert_eq!(sub(&tool).3, Some("Read a.rs"));
        assert_eq!(
            (tool.state, tool.activity),
            (Some(AgentState::Working), None)
        );
        let ask = child(&json!({
            "hook_event_name": "PermissionRequest", "agent_id": "a1",
            "tool_name": "Bash", "tool_input": {"command": "ls"}
        }));
        assert_eq!(
            sub(&ask),
            (
                "a1",
                None,
                Some(AgentState::Blocked),
                Some("Allow Bash ls?")
            )
        );
        assert_eq!(ask.state, Some(AgentState::Blocked));
    }

    #[test]
    fn a_subagent_finishing_leaves_the_parent_as_it_is() {
        let stop = child(&json!({
            "hook_event_name": "SubagentStop", "agent_id": "a1", "agent_type": "Explore",
            "last_assistant_message": "Found 3 issues", "background_tasks": []
        }));
        assert_eq!(sub(&stop).2, Some(AgentState::Done));
        assert_eq!((stop.state, stop.subagents_running), (None, None));
    }

    #[test]
    fn a_turns_end_counts_the_subagents_still_running() {
        let stop = |tasks: Value| {
            child(&json!({"hook_event_name": "Stop", "background_tasks": tasks})).subagents_running
        };
        assert_eq!(stop(json!([])), Some(0));
        let mixed = json!([
            {"id": "t1", "type": "shell", "command": "tail -f log"},
            {"id": "t2", "type": "subagent", "agent_type": "Explore"},
        ]);
        assert_eq!(stop(mixed), Some(1));
        let old = child(&json!({"hook_event_name": "Stop"}));
        assert_eq!(old.subagents_running, None, "older agents don't say");
        let main =
            child(&json!({"hook_event_name": "UserPromptSubmit", "agent_id": "", "prompt": "hi"}));
        assert_eq!(main.subagent, None, "an empty id is the main agent");
        let unknown = json!({"hook_event_name": "TaskCreated", "agent_id": "a1"});
        assert!(
            AgentHook::new(AgentKind::Claude, &unknown)
                .report(None)
                .is_none()
        );
    }
}
