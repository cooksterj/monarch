//! `[agents.<id>]`: how to launch, resume and quick-answer each agent.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Launch configuration for one agent kind.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Executable to run.
    pub command: String,
    /// Arguments for a fresh session. `{session_id}` expands to a UUID
    /// Monarch generates so the session can be resumed later.
    pub args: Vec<String>,
    /// Arguments to resume a session; `{session_id}` expands. Empty means
    /// the agent cannot be resumed and restarts fresh after a server restart.
    pub resume_args: Vec<String>,
    /// Inbox quick answers: key → bytes sent to the pane (e.g. `"y" = "1"`).
    pub quick_answers: BTreeMap<String, String>,
}

impl AgentConfig {
    /// The agents Monarch knows how to launch out of the box.
    #[must_use]
    pub fn builtin() -> BTreeMap<String, Self> {
        BTreeMap::from([
            ("claude".to_owned(), Self::claude()),
            ("codex".to_owned(), Self::codex()),
        ])
    }

    fn claude() -> Self {
        Self {
            command: "claude".into(),
            args: strings(&["--session-id", "{session_id}"]),
            resume_args: strings(&["--resume", "{session_id}"]),
            quick_answers: answers(&[("y", "1"), ("n", "\x1b")]),
        }
    }

    fn codex() -> Self {
        Self {
            command: "codex".into(),
            args: Vec::new(),
            resume_args: Vec::new(),
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

    /// Expands `{session_id}` in `args` (or `resume_args` when resuming).
    #[must_use]
    pub fn argv(&self, session_id: &str, resume: bool) -> Vec<String> {
        let template = if resume {
            &self.resume_args
        } else {
            &self.args
        };
        template
            .iter()
            .map(|a| a.replace("{session_id}", session_id))
            .collect()
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
    fn claude_argv_expands_session_id() {
        let claude = AgentConfig::claude();
        assert_eq!(claude.argv("abc", false), ["--session-id", "abc"]);
        assert_eq!(claude.argv("abc", true), ["--resume", "abc"]);
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
    fn codex_has_no_resume_template_yet() {
        assert!(AgentConfig::codex().argv("abc", true).is_empty());
    }
}
