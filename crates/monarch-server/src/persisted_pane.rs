//! One pane as written to the state file, and how it comes back.

use std::path::PathBuf;

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use crate::{LaunchConfig, PaneAgent, PaneSpec};

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

    /// The pane to start in its place. An agent resumes its session as
    /// `launch` says (or, with `resume_agents` off, leaves a shell in its
    /// directory); a shell comes back as a shell in the same directory.
    /// Any other command also comes back as a shell there, never re-run:
    /// it may not be safe to run twice (a deploy, a delete).
    #[must_use]
    pub fn restore(&self, launch: &LaunchConfig) -> PaneSpec {
        let (argv, agent) = match &self.agent {
            Some(agent) if launch.resume_agents => {
                let (argv, agent) = agent.restore(&launch.agent(agent.kind));
                (argv, Some(agent))
            }
            Some(_) => (Vec::new(), None),
            None if self.is_shell() => (self.argv.clone(), None),
            None => (Vec::new(), None),
        };
        PaneSpec {
            argv,
            cwd: self.cwd.clone().filter(|dir| dir.is_dir()),
            agent,
        }
    }

    /// The line shown above the restored pane's first output.
    #[must_use]
    pub fn banner(&self, launch: &LaunchConfig) -> String {
        let mut text = String::from("\x1b[2mmonarch: restored after the server stopped");
        if let Some(agent) = &self.agent {
            text.push_str("; ");
            if launch.resume_agents {
                text.push_str(&agent.resume_note(&launch.agent(agent.kind)));
            } else {
                text.push_str("it was running ");
                text.push_str(agent.kind.display_name());
                text.push_str(" (resume_agents is off)");
            }
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
    use monarch_core::{AgentConfig, AgentKind};

    use super::*;

    fn launch() -> LaunchConfig {
        LaunchConfig::default()
    }

    fn claude_pane() -> (PersistedPane, String) {
        let spec = PaneSpec::agent(AgentKind::Claude, &AgentConfig::for_kind(AgentKind::Claude));
        let id = spec
            .agent
            .as_ref()
            .and_then(|a| a.session_id.clone())
            .expect("id");
        (PersistedPane::new(&spec, None), id)
    }

    #[test]
    fn a_shell_comes_back_in_its_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pane = PersistedPane::new(&PaneSpec::default(), Some(dir.path().into()));
        let spec = pane.restore(&launch());
        assert!(spec.argv.is_empty());
        assert_eq!(spec.cwd.as_deref(), Some(dir.path()));
        assert!(!pane.banner(&launch()).contains("running"));
    }

    #[test]
    fn a_command_comes_back_as_a_shell_and_is_named_in_the_banner() {
        let pane = PersistedPane::new(&PaneSpec::shell_command("make deploy"), None);
        assert!(pane.restore(&launch()).argv.is_empty(), "never re-run");
        assert!(
            pane.banner(&launch())
                .contains("it was running: make deploy")
        );
    }

    #[test]
    fn an_agent_resumes_its_session() {
        let (pane, id) = claude_pane();
        let back = pane.restore(&launch());
        assert_eq!(back.argv, ["claude", "--resume", id.as_str()]);
        assert_eq!(back.agent, pane.agent, "still the same session next time");
        assert!(
            pane.banner(&launch())
                .contains(&format!("resuming Claude Code session {id}"))
        );
    }

    #[test]
    fn an_agent_resumes_with_the_configured_command() {
        let (pane, id) = claude_pane();
        let mut launch = launch();
        let claude = launch.agents.get_mut(&AgentKind::Claude).expect("claude");
        claude.command = "my-claude".into();
        claude.resume_args = vec!["-r".into(), "{session_id}".into()];
        assert_eq!(pane.restore(&launch).argv, ["my-claude", "-r", id.as_str()]);
    }

    #[test]
    fn with_resume_off_an_agent_pane_is_a_shell_in_its_directory() {
        let (pane, _) = claude_pane();
        let launch = LaunchConfig {
            resume_agents: false,
            ..launch()
        };
        let back = pane.restore(&launch);
        assert!(back.argv.is_empty() && back.agent.is_none());
        let banner = pane.banner(&launch);
        assert!(banner.contains("it was running Claude Code"), "{banner}");
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
        assert_eq!(pane.restore(&launch()).cwd, None);
    }

    #[test]
    fn control_characters_never_reach_the_banner() {
        let pane = PersistedPane {
            argv: vec!["sh".into(), "-c".into(), "echo \x1b]2;x\x07".into()],
            ..PersistedPane::default()
        };
        assert!(!pane.banner(&launch()).contains('\x07'));
    }
}
