//! An agent Monarch launched in a pane, and how to resume its session.

use monarch_core::{AgentConfig, AgentKind};
use serde::{Deserialize, Serialize};

use crate::AgentHistory;

/// Which agent runs in a pane and, once known, its session id.
///
/// The commands come from `[agents.<id>]` in the config. By default Claude
/// Code accepts a chosen id (`--session-id <uuid>`), so Monarch picks one
/// at launch and resumes it with `claude --resume <uuid>`. Codex picks its
/// own id; its hooks report it (`session_id`, from M3), and `codex resume
/// <id>` reopens it. Until then a Codex pane resumes through `codex
/// resume`'s picker, which lists that directory's sessions, rather than
/// guessing with `--last` (another pane's session in the same directory
/// may be newer).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneAgent {
    /// Which agent.
    pub kind: AgentKind,
    /// The agent's own session id, when known.
    pub session_id: Option<String>,
    /// Whether it has a conversation to resume, as its hooks tell.
    #[serde(default)]
    pub history: AgentHistory,
}

impl PaneAgent {
    /// A new session of `kind`, with its id chosen now when `config`'s
    /// launch arguments ask for one.
    #[must_use]
    pub fn new_session(kind: AgentKind, config: &AgentConfig) -> Self {
        let session_id = config
            .chooses_session_id()
            .then(|| uuid::Uuid::new_v4().to_string());
        Self {
            kind,
            session_id,
            history: AgentHistory::Unknown,
        }
    }

    /// The command that starts this session for the first time.
    #[must_use]
    pub fn launch_argv(&self, config: &AgentConfig) -> Vec<String> {
        config.launch_argv(self.session_id.as_deref())
    }

    /// The command that resumes this session, if `config` has one.
    #[must_use]
    pub fn resume_argv(&self, config: &AgentConfig) -> Option<Vec<String>> {
        config.resume_argv(self.session_id.as_deref())
    }

    /// The pane that replaces this one after a restart, as a command and
    /// the agent it then runs: resume the conversation; start afresh with
    /// the same id when its hooks say it never had one; or, when `config`
    /// cannot resume, a new session.
    #[must_use]
    pub fn restore(&self, config: &AgentConfig) -> (Vec<String>, Self) {
        if !self.history.resumable() {
            return (self.launch_argv(config), self.clone());
        }
        match self.resume_argv(config) {
            Some(argv) => (argv, self.clone()),
            None => {
                let fresh = Self::new_session(self.kind, config);
                (fresh.launch_argv(config), fresh)
            }
        }
    }

    /// What the restore notice says about this pane.
    #[must_use]
    pub fn resume_note(&self, config: &AgentConfig) -> String {
        let name = self.kind.display_name();
        if !self.history.resumable() {
            return format!("starting {name} again (it had no conversation yet)");
        }
        if config.resume_args.is_empty() {
            return format!("starting a new {name} session (resume_args is empty)");
        }
        match &self.session_id {
            Some(id) => format!("resuming {name} session {id}"),
            None => format!("pick the {name} session to resume"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin(kind: AgentKind) -> AgentConfig {
        AgentConfig::for_kind(kind)
    }

    #[test]
    fn claude_launches_with_a_chosen_id_and_resumes_it() {
        let config = builtin(AgentKind::Claude);
        let agent = PaneAgent::new_session(AgentKind::Claude, &config);
        let id = agent.session_id.clone().expect("chosen at launch");
        assert!(uuid::Uuid::parse_str(&id).is_ok(), "claude requires a UUID");
        assert_eq!(
            agent.launch_argv(&config),
            ["claude", "--session-id", id.as_str()]
        );
        assert_eq!(
            agent.resume_argv(&config).expect("resumable"),
            ["claude", "--resume", id.as_str()]
        );
        assert!(agent.resume_note(&config).contains(&id));
    }

    #[test]
    fn each_claude_session_gets_its_own_id() {
        let config = builtin(AgentKind::Claude);
        let a = PaneAgent::new_session(AgentKind::Claude, &config);
        let b = PaneAgent::new_session(AgentKind::Claude, &config);
        assert_ne!(a.session_id, b.session_id);
    }

    #[test]
    fn codex_without_an_id_resumes_through_the_picker() {
        let config = builtin(AgentKind::Codex);
        let agent = PaneAgent::new_session(AgentKind::Codex, &config);
        assert_eq!(agent.session_id, None, "codex picks its own");
        assert_eq!(agent.launch_argv(&config), ["codex"]);
        assert_eq!(
            agent.resume_argv(&config).expect("resumable"),
            ["codex", "resume"],
            "never --last"
        );
        assert!(
            agent
                .resume_note(&config)
                .starts_with("pick the Codex session")
        );
    }

    #[test]
    fn codex_with_a_reported_id_resumes_it() {
        let agent = PaneAgent {
            kind: AgentKind::Codex,
            session_id: Some("0199a213-81c0-7800-8aa1-bbab2a035a53".into()),
            history: AgentHistory::Started,
        };
        assert_eq!(
            agent
                .resume_argv(&builtin(AgentKind::Codex))
                .expect("resumable"),
            ["codex", "resume", "0199a213-81c0-7800-8aa1-bbab2a035a53"]
        );
    }

    #[test]
    fn a_pane_never_prompted_starts_afresh_with_the_same_id() {
        let config = builtin(AgentKind::Claude);
        let mut agent = PaneAgent::new_session(AgentKind::Claude, &config);
        let id = agent.session_id.clone().expect("chosen");
        assert_eq!(
            agent.restore(&config).0,
            agent.resume_argv(&config).expect("resumable"),
            "no hooks: resume"
        );
        agent.history = AgentHistory::Empty;
        let (argv, same) = agent.restore(&config);
        assert_eq!(argv, ["claude", "--session-id", id.as_str()]);
        assert_eq!(same, agent);
        assert!(agent.resume_note(&config).contains("no conversation yet"));
        let old = r#"{"kind":"claude","session_id":"x"}"#;
        let read: PaneAgent = serde_json::from_str(old).expect("older state file");
        assert_eq!(read.history, AgentHistory::Unknown);
    }

    #[test]
    fn without_resume_args_a_restore_is_a_new_session() {
        let config = AgentConfig {
            resume_args: Vec::new(),
            ..builtin(AgentKind::Claude)
        };
        let agent = PaneAgent {
            history: AgentHistory::Started,
            ..PaneAgent::new_session(AgentKind::Claude, &config)
        };
        let (argv, fresh) = agent.restore(&config);
        assert_ne!(fresh.session_id, agent.session_id, "the old id is taken");
        assert_eq!(argv[..2], ["claude", "--session-id"]);
        assert_eq!(Some(&argv[2]), fresh.session_id.as_ref());
        assert!(
            agent
                .resume_note(&config)
                .contains("new Claude Code session")
        );
    }

    #[test]
    fn a_configured_wrapper_launches_with_its_flags() {
        let config = AgentConfig {
            command: "my-codex".into(),
            args: vec!["--full-auto".into()],
            ..builtin(AgentKind::Codex)
        };
        let agent = PaneAgent::new_session(AgentKind::Codex, &config);
        assert_eq!(agent.launch_argv(&config), ["my-codex", "--full-auto"]);
    }
}
