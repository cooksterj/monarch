//! One pane as written to the state file, and how it comes back.

use std::path::PathBuf;

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use crate::{PaneAgent, PaneSpec};

/// What a pane ran and where, enough to start it again after the server
/// died.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedPane {
    /// Its id when saved, which the saved tabs name it by; `None` in
    /// version 1 files.
    #[serde(default)]
    pub id: Option<PaneId>,
    /// Program and arguments as started; empty for the user's shell.
    pub argv: Vec<String>,
    /// The program's working directory when last seen.
    pub cwd: Option<PathBuf>,
    /// The agent session the pane ran, if Monarch launched it as one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<PaneAgent>,
}

impl PersistedPane {
    /// Records `spec`, with `cwd` (the live directory) if known.
    #[must_use]
    pub fn new(spec: &PaneSpec, cwd: Option<PathBuf>) -> Self {
        Self {
            id: None,
            argv: spec.argv.clone(),
            cwd: cwd.or_else(|| spec.cwd.clone()),
            agent: spec.agent.clone(),
        }
    }

    /// The pane to start in its place. An agent resumes its session; a
    /// shell comes back as a shell in the same directory. Any other command
    /// also comes back as a shell there, never re-run: it may not be safe
    /// to run twice (a deploy, a delete).
    #[must_use]
    pub fn restore(&self) -> PaneSpec {
        let argv = match &self.agent {
            Some(agent) => agent.resume_argv(),
            None if self.is_shell() => self.argv.clone(),
            None => Vec::new(),
        };
        PaneSpec {
            argv,
            cwd: self.cwd.clone().filter(|dir| dir.is_dir()),
            agent: self.agent.clone(),
        }
    }

    /// The line shown above the restored pane's first output.
    #[must_use]
    pub fn banner(&self) -> String {
        let mut text = String::from("\x1b[2mmonarch: restored after the server stopped");
        if let Some(agent) = &self.agent {
            text.push_str("; ");
            text.push_str(&agent.resume_note());
        } else if !self.is_shell() {
            text.push_str("; it was running: ");
            text.push_str(&self.command_text());
        }
        text.push_str("\x1b[0m\r\n");
        text
    }

    /// A shell spec has no arguments: the configured shell or the default.
    fn is_shell(&self) -> bool {
        self.argv.len() <= 1
    }

    /// The command as the user typed it (`sh -c` text unwrapped).
    fn command_text(&self) -> String {
        match self.argv.as_slice() {
            [_, flag, text] if flag == "-c" => text.clone(),
            argv => argv.join(" "),
        }
        .chars()
        .filter(|c| !c.is_control())
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentKind;

    use super::*;

    #[test]
    fn a_shell_comes_back_in_its_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pane = PersistedPane::new(&PaneSpec::default(), Some(dir.path().into()));
        let spec = pane.restore();
        assert!(spec.argv.is_empty());
        assert_eq!(spec.cwd.as_deref(), Some(dir.path()));
        assert!(!pane.banner().contains("running"));
    }

    #[test]
    fn a_command_comes_back_as_a_shell_and_is_named_in_the_banner() {
        let pane = PersistedPane::new(&PaneSpec::shell_command("make deploy"), None);
        assert!(pane.restore().argv.is_empty(), "never re-run");
        assert!(pane.banner().contains("it was running: make deploy"));
    }

    #[test]
    fn an_agent_resumes_its_session() {
        let spec = PaneSpec::agent(AgentKind::Claude);
        let id = spec
            .agent
            .as_ref()
            .and_then(|a| a.session_id.clone())
            .expect("id");
        let pane = PersistedPane::new(&spec, None);
        let back = pane.restore();
        assert_eq!(back.argv, ["claude", "--resume", id.as_str()]);
        assert_eq!(back.agent, spec.agent, "still the same session next time");
        assert!(
            pane.banner()
                .contains(&format!("resuming Claude Code session {id}"))
        );
    }

    #[test]
    fn files_without_agents_still_load() {
        let pane: PersistedPane = serde_json::from_str(r#"{"argv":[],"cwd":null}"#).expect("parse");
        assert_eq!(pane.agent, None);
    }

    #[test]
    fn a_vanished_directory_is_dropped() {
        let pane = PersistedPane {
            cwd: Some("/nonexistent/monarch/dir".into()),
            ..PersistedPane::default()
        };
        assert_eq!(pane.restore().cwd, None);
    }

    #[test]
    fn control_characters_never_reach_the_banner() {
        let pane = PersistedPane {
            argv: vec!["sh".into(), "-c".into(), "echo \x1b]2;x\x07".into()],
            ..PersistedPane::default()
        };
        assert!(!pane.banner().contains('\x07'));
    }
}
