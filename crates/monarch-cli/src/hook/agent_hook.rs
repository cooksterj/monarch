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
use monarch_proto::{AgentReport, BackgroundTask, SubagentReport};
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
            subagent: self.launched(),
            background: self.background(),
            user_prompt: self.field("hook_event_name") == Some("UserPromptSubmit")
                && self.injected().is_none(),
            ..base
        })
    }

    /// A prompt the agent sent itself, with what to show for it: Claude
    /// wakes its own turn with a background sub-agent's or task's result
    /// (`<agent-message from="…">`, `<task-notification>`) or a teammate's
    /// message (`Another Claude session sent a message: <teammate-message …>`)
    /// through the same hook as the user's prompts.
    fn injected(&self) -> Option<&'static str> {
        const PEER: &str = "Another Claude session sent a message:";
        let prompt = self.field("prompt")?.trim_start();
        let from_peer = prompt.strip_prefix(PEER).map(str::trim_start);
        let message = from_peer.unwrap_or(prompt);
        if prompt.starts_with("<agent-message") {
            Some("Reading a sub-agent's report")
        } else if prompt.starts_with("<task-notification>") {
            Some("Reading a background task's result")
        } else if message.starts_with("<teammate-message") {
            Some("Reading a teammate's message")
        } else if from_peer.is_some() {
            Some("Reading another session's message")
        } else {
            None
        }
    }

    /// An event inside sub-agent `id`: its own state and line, and the
    /// parent's state (working for it, or blocked on its question).
    fn subagent_report(&self, id: &str, base: AgentReport) -> Option<AgentReport> {
        let text = |key: &str| self.field(key).map(str::to_owned);
        let event = self.field("hook_event_name")?;
        let (parent, state, activity) = match event {
            // Codex children get their task, and any follow-up, as a prompt.
            "SubagentStart" | "UserPromptSubmit" => {
                (None, AgentState::Working, Some(String::new()))
            }
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
                label: (event == "UserPromptSubmit").then(|| self.task()).flatten(),
                state: Some(state),
                activity,
            }),
            ..base
        })
    }

    /// A sub-agent's prompt as the name of its row: its first line.
    fn task(&self) -> Option<String> {
        let line = self
            .field("prompt")?
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())?;
        Some(line.to_owned())
    }

    /// The sub-agent the parent's Agent tool just started or ran, named
    /// by its task: `PostToolUse` returns its `agentId` and the
    /// `description` it was given. A background launch is running; a
    /// foreground one has finished by then, so its report only names it.
    fn launched(&self) -> Option<SubagentReport> {
        if self.field("hook_event_name") != Some("PostToolUse")
            || self.field("tool_name") != Some("Agent")
        {
            return None;
        }
        let text = |v: Option<&Value>, key: &str| {
            let found = v?.get(key)?.as_str()?.trim();
            (!found.is_empty()).then(|| found.to_owned())
        };
        let response = self.input.get("tool_response");
        let label = text(response, "description")
            .or_else(|| text(self.input.get("tool_input"), "description"))?;
        let running = text(response, "status").as_deref() == Some("async_launched");
        Some(SubagentReport {
            id: text(response, "agentId")?,
            kind: None,
            label: Some(label),
            state: running.then_some(AgentState::Working),
            activity: None,
        })
    }

    /// A `Stop`'s `background_tasks`, each with an id (`None` when the
    /// event carries no list).
    fn background(&self) -> Option<Vec<BackgroundTask>> {
        let tasks = self.input.get("background_tasks")?.as_array()?;
        let text = |t: &Value, key: &str| t.get(key).and_then(Value::as_str).map(str::to_owned);
        let listed = tasks.iter().filter_map(|t| {
            Some(BackgroundTask {
                id: text(t, "id")?,
                kind: text(t, "type").unwrap_or_default(),
            })
        });
        Some(listed.collect())
    }

    /// The event's state and new activity line (`""` clears it).
    fn meaning(&self) -> Option<(AgentState, Option<String>)> {
        let text = |key: &str| self.field(key).map(str::to_owned);
        Some(match self.field("hook_event_name")? {
            "SessionStart" => (AgentState::Idle, Some(String::new())),
            "UserPromptSubmit" => match self.injected() {
                Some(line) => (AgentState::Working, Some(line.into())),
                None => (AgentState::Working, text("prompt")),
            },
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
    fn a_codex_childs_prompt_names_its_row_and_starts_it() {
        // As Codex's `UserPromptSubmitCommandInput` serialises it inside a
        // child `spawn_agent` started: `agent_id` is the child's thread.
        let input = json!({
            "hook_event_name": "UserPromptSubmit", "session_id": "s", "turn_id": "t",
            "agent_id": "019a-child", "agent_type": "default", "cwd": "/w",
            "prompt": "\n  Run wc -l a.txt and report the count.\nBe brief.",
            "permission_mode": "default", "transcript_path": null, "model": "gpt"
        });
        let prompt = AgentHook::new(AgentKind::Codex, &input)
            .report(Some(PaneId(4)))
            .expect("report");
        assert_eq!(
            sub(&prompt),
            (
                "019a-child",
                Some("default"),
                Some(AgentState::Working),
                Some("")
            )
        );
        let label = prompt.subagent.as_ref().and_then(|s| s.label.as_deref());
        assert_eq!(label, Some("Run wc -l a.txt and report the count."));
        assert_eq!(
            (prompt.state, prompt.user_prompt),
            (None, false),
            "not the user's"
        );
        let blank = child(&json!({
            "hook_event_name": "UserPromptSubmit", "agent_id": "c", "prompt": "  \n"
        }));
        let unnamed = blank.subagent.as_ref().and_then(|s| s.label.as_deref());
        assert_eq!(unnamed, None, "a blank prompt names nothing");
    }

    #[test]
    fn a_subagent_finishing_leaves_the_parent_as_it_is() {
        let stop = child(&json!({
            "hook_event_name": "SubagentStop", "agent_id": "a1", "agent_type": "Explore",
            "last_assistant_message": "Found 3 issues", "background_tasks": []
        }));
        assert_eq!(sub(&stop).2, Some(AgentState::Done));
        assert_eq!((stop.state, stop.background), (None, None));
    }

    #[test]
    fn a_turns_end_lists_what_still_runs_in_the_background() {
        let stop = |tasks: Value| {
            child(&json!({"hook_event_name": "Stop", "background_tasks": tasks})).background
        };
        assert_eq!(stop(json!([])), Some(Vec::new()));
        let mixed = json!([
            {"id": "t1", "type": "shell", "command": "tail -f log"},
            {"id": "a2", "type": "local_agent", "agent_type": "Explore"},
            {"type": "monitor"},
        ]);
        let task = |id: &str, kind: &str| BackgroundTask {
            id: id.into(),
            kind: kind.into(),
        };
        assert_eq!(
            stop(mixed),
            Some(vec![task("t1", "shell"), task("a2", "local_agent")]),
            "every task with an id, with its label"
        );
        let old = child(&json!({"hook_event_name": "Stop"}));
        assert_eq!(old.background, None, "older agents don't say");
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

    #[test]
    fn prompts_the_agent_sends_itself_are_not_the_users() {
        let prompt =
            |text: &str| child(&json!({"hook_event_name": "UserPromptSubmit", "prompt": text}));
        let typed = prompt("fix the bug");
        assert!(typed.user_prompt);
        let report = prompt("<agent-message from=\"aefd56\">\nFound 3 TODOs");
        assert!(!report.user_prompt, "a sub-agent reporting back");
        assert_eq!(
            report.activity.as_deref(),
            Some("Reading a sub-agent's report")
        );
        assert_eq!(report.state, Some(AgentState::Working));
        let task = prompt("\n<task-notification>\n<task-id>a1</task-id>");
        assert!(!task.user_prompt, "a background task finishing");
        let teammate = prompt(
            "Another Claude session sent a message:\n\
             <teammate-message teammate_id=\"timer-1\">\nagent 1 done\n</teammate-message>",
        );
        assert!(!teammate.user_prompt, "a teammate reporting back");
        assert_eq!(
            teammate.activity.as_deref(),
            Some("Reading a teammate's message")
        );
        let peer = prompt("Another Claude session sent a message:\nhello");
        assert!(!peer.user_prompt, "another session, not the user");
        let quoted = prompt("why does it say Another Claude session sent a message:?");
        assert!(quoted.user_prompt, "only a prompt that starts with it");
        let tool = child(&json!({"hook_event_name": "PreToolUse", "tool_name": "Read"}));
        assert!(!tool.user_prompt, "only prompts count");
    }

    #[test]
    fn the_agent_tool_names_the_sub_agent_it_started() {
        let launched = |status: &str| {
            json!({
                "hook_event_name": "PostToolUse", "tool_name": "Agent",
                "tool_input": {"description": "Timer agent 1", "name": "timer-1"},
                "tool_response": {"status": status, "agentId": "a024", "description": "Timer agent 1"}
            })
        };
        let report = child(&launched("async_launched"));
        assert_eq!(
            report.state,
            Some(AgentState::Working),
            "the parent's own state"
        );
        assert_eq!(report.activity, None);
        let sub = report.subagent.expect("names the sub-agent");
        assert_eq!(
            (sub.id.as_str(), sub.label.as_deref(), sub.state),
            ("a024", Some("Timer agent 1"), Some(AgentState::Working))
        );
        let ran = child(&launched("completed")).subagent.expect("names it");
        assert_eq!(ran.state, None, "a foreground agent has already finished");
        let other = json!({"hook_event_name": "PostToolUse", "tool_name": "Bash",
            "tool_response": {"agentId": "x", "description": "y"}});
        assert_eq!(child(&other).subagent, None, "only the Agent tool");
        let nameless = json!({"hook_event_name": "PostToolUse", "tool_name": "Agent",
            "tool_response": {"status": "async_launched", "agentId": "a1"}});
        assert_eq!(child(&nameless).subagent, None, "nothing to name it by");
        let started = json!({"hook_event_name": "PreToolUse", "tool_name": "Agent",
            "tool_input": {"description": "Timer agent 1"}});
        assert_eq!(child(&started).subagent, None, "no id until it returns");
    }

    /// Every hook event Claude Code has (its settings schema, 2026-11).
    const CLAUDE_HOOK_EVENTS: &[&str] = &[
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "PostToolBatch",
        "Notification",
        "UserPromptSubmit",
        "UserPromptExpansion",
        "SessionStart",
        "SessionEnd",
        "Stop",
        "StopFailure",
        "SubagentStart",
        "SubagentStop",
        "PreCompact",
        "PostCompact",
        "PreModelSwitch",
        "PostModelSwitch",
        "PermissionRequest",
        "PermissionDenied",
        "Setup",
        "TeammateIdle",
        "TaskCreated",
        "TaskCompleted",
        "Elicitation",
        "ElicitationResult",
        "ConfigChange",
        "WorktreeCreate",
        "WorktreeRemove",
        "InstructionsLoaded",
        "CwdChanged",
        "FileChanged",
        "DirectoryAdded",
        "MessageDisplay",
    ];

    #[test]
    fn every_event_that_reports_is_one_setup_installs() {
        let installed = crate::HookCommand::Claude.events();
        for event in CLAUDE_HOOK_EVENTS {
            let main = json!({"hook_event_name": event, "notification_type": "permission_prompt"});
            let child = json!({"hook_event_name": event, "agent_id": "a1",
                "notification_type": "permission_prompt"});
            let reports = [main, child].iter().any(|input| {
                AgentHook::new(AgentKind::Claude, input)
                    .report(None)
                    .is_some()
            });
            assert!(
                !reports || installed.contains(event),
                "`monarch hook claude` reads {event}, but `monarch setup` doesn't install it"
            );
        }
        for event in installed {
            assert!(
                CLAUDE_HOOK_EVENTS.contains(event),
                "{event} isn't a Claude event"
            );
        }
    }
}
