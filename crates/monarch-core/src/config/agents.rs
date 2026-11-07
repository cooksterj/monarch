//! `[agents.<id>]`: how to launch, resume and quick-answer each agent.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::AgentKind;

/// The placeholder in `args` and `resume_args` that becomes the session id.
pub const SESSION_ID: &str = "{session_id}";

/// Launch configuration for one agent kind.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Executable to run.
    pub command: String,
    /// Arguments for a fresh session. When `{session_id}` appears here
    /// Monarch chooses a UUID at launch and expands it, so the session can
    /// be resumed later; otherwise the agent picks its own id.
    pub args: Vec<String>,
    /// Arguments to resume a session. `{session_id}` expands to the id
    /// known for the pane; an argument mentioning it is left out when no
    /// id is known. Empty means the agent cannot be resumed and starts
    /// afresh after a server restart.
    pub resume_args: Vec<String>,
    /// Inbox quick answers: key → bytes sent to the pane (e.g. `"y" = "1"`).
    pub quick_answers: BTreeMap<String, String>,
}

impl AgentConfig {
    /// The agents Monarch knows how to launch out of the box.
    #[must_use]
    pub fn builtin() -> BTreeMap<String, Self> {
        AgentKind::ALL
            .into_iter()
            .map(|kind| (kind.name().to_owned(), Self::for_kind(kind)))
            .collect()
    }

    /// The built-in configuration of `kind`.
    #[must_use]
    pub fn for_kind(kind: AgentKind) -> Self {
        match kind {
            AgentKind::Claude => Self::claude(),
            AgentKind::Codex => Self::codex(),
        }
    }

    fn claude() -> Self {
        Self {
            command: "claude".into(),
            args: strings(&["--session-id", SESSION_ID]),
            resume_args: strings(&["--resume", SESSION_ID]),
            quick_answers: answers(&[("y", "1"), ("n", "\x1b")]),
        }
    }

    /// Codex chooses its own id (its hooks report it); without one,
    /// `codex resume` opens its picker of the directory's sessions.
    fn codex() -> Self {
        Self {
            command: "codex".into(),
            args: Vec::new(),
            resume_args: strings(&["resume", SESSION_ID]),
            quick_answers: answers(&[("y", "y"), ("n", "n")]),
        }
    }

    /// What the inbox sends to this agent's pane for `key`, if the key is
    /// a quick answer (`y` on Claude Code presses `1`); `None` means type
    /// the key as it is.
    #[must_use]
    pub fn quick_answer(&self, key: char) -> Option<&str> {
        let mut buf = [0; 4];
        self.quick_answers
            .get(key.encode_utf8(&mut buf) as &str)
            .map(String::as_str)
    }

    /// Whether Monarch must choose a session id at launch: the launch
    /// arguments mention one.
    #[must_use]
    pub fn chooses_session_id(&self) -> bool {
        self.args.iter().any(|a| a.contains(SESSION_ID))
    }

    /// The command that starts a fresh session: `command` then `args`
    /// with `{session_id}` expanded (left out when `session_id` is `None`).
    #[must_use]
    pub fn launch_argv(&self, session_id: Option<&str>) -> Vec<String> {
        self.argv(&self.args, session_id)
    }

    /// The command that resumes a session, or `None` when this agent has
    /// no resume arguments.
    #[must_use]
    pub fn resume_argv(&self, session_id: Option<&str>) -> Option<Vec<String>> {
        (!self.resume_args.is_empty()).then(|| self.argv(&self.resume_args, session_id))
    }

    /// The executable's file name, as a process list shows it.
    #[must_use]
    pub fn program_name(&self) -> &str {
        self.command.rsplit('/').next().unwrap_or(&self.command)
    }

    /// Problems with this configuration of `id` (each falls back to the
    /// built-in agent).
    #[must_use]
    pub fn warnings(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        if id.parse::<AgentKind>().is_err() {
            out.push(format!(
                "`agents.{id}` is not an agent Monarch can launch ({}); ignored",
                AgentKind::choices()
            ));
        } else if self.command.trim().is_empty() {
            out.push(format!(
                "`agents.{id}.command` is empty; the built-in `{id}` is used"
            ));
        }
        out
    }

    fn argv(&self, template: &[String], session_id: Option<&str>) -> Vec<String> {
        let mut argv = vec![self.command.clone()];
        for arg in template {
            match session_id {
                _ if !arg.contains(SESSION_ID) => argv.push(arg.clone()),
                Some(id) => argv.push(arg.replace(SESSION_ID, id)),
                None => {}
            }
        }
        argv
    }
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn answers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_chooses_an_id_and_expands_it() {
        let claude = AgentConfig::claude();
        assert!(claude.chooses_session_id());
        assert_eq!(
            claude.launch_argv(Some("abc")),
            ["claude", "--session-id", "abc"]
        );
        assert_eq!(
            claude.resume_argv(Some("abc")).expect("resumable"),
            ["claude", "--resume", "abc"]
        );
    }

    #[test]
    fn codex_resumes_with_an_id_or_through_its_picker() {
        let codex = AgentConfig::codex();
        assert!(!codex.chooses_session_id());
        assert_eq!(codex.launch_argv(None), ["codex"]);
        assert_eq!(
            codex.resume_argv(Some("x")).expect("resumable"),
            ["codex", "resume", "x"]
        );
        assert_eq!(
            codex.resume_argv(None).expect("resumable"),
            ["codex", "resume"],
            "an argument needing an id is left out"
        );
    }

    #[test]
    fn empty_resume_args_mean_no_resume() {
        let agent = AgentConfig {
            resume_args: Vec::new(),
            ..AgentConfig::claude()
        };
        assert_eq!(agent.resume_argv(Some("abc")), None);
    }

    #[test]
    fn a_wrapper_keeps_its_flags_and_names_its_program() {
        let agent = AgentConfig {
            command: "/opt/bin/my-claude".into(),
            args: strings(&["--model", "opus", "--session-id", SESSION_ID]),
            ..AgentConfig::claude()
        };
        assert_eq!(
            agent.launch_argv(Some("id")),
            [
                "/opt/bin/my-claude",
                "--model",
                "opus",
                "--session-id",
                "id"
            ]
        );
        assert_eq!(agent.program_name(), "my-claude");
        assert_eq!(AgentConfig::codex().program_name(), "codex");
    }

    #[test]
    fn quick_answers_map_keys_and_leave_the_rest() {
        let claude = AgentConfig::claude();
        assert_eq!(claude.quick_answer('y'), Some("1"));
        assert_eq!(claude.quick_answer('n'), Some("\x1b"));
        assert_eq!(claude.quick_answer('2'), None, "typed as it is");
        assert_eq!(AgentConfig::codex().quick_answer('y'), Some("y"));
        assert_eq!(AgentConfig::default().quick_answer('y'), None);
    }

    #[test]
    fn warnings_name_unknown_agents_and_empty_commands() {
        assert!(AgentConfig::claude().warnings("claude").is_empty());
        let blank = AgentConfig {
            command: "  ".into(),
            ..AgentConfig::claude()
        };
        assert_eq!(blank.warnings("claude").len(), 1);
        assert!(blank.warnings("claude")[0].contains("built-in"));
        let unknown = AgentConfig::claude().warnings("aider");
        assert!(unknown[0].contains("not an agent"), "{unknown:?}");
    }

    #[test]
    fn builtin_covers_every_kind() {
        let builtin = AgentConfig::builtin();
        for kind in AgentKind::ALL {
            assert_eq!(builtin[kind.name()], AgentConfig::for_kind(kind));
        }
    }
}
