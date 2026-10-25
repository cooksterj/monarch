//! An agent Monarch launched in a pane, and how to resume its session.

use monarch_core::AgentKind;
use serde::{Deserialize, Serialize};

use crate::AgentHistory;

/// Which agent runs in a pane and, once known, its session id.
///
/// Claude Code accepts a chosen id (`--session-id <uuid>`), so Monarch
/// picks one at launch and resumes it with `claude --resume <uuid>`.
/// Codex picks its own id; its hooks report it (`session_id`, from M3), and
/// `codex resume <id>` reopens it. Until then a Codex pane resumes through
/// `codex resume`'s picker, which lists that directory's sessions, rather
/// than guessing with `--last` (another pane's session in the same
/// directory may be newer).
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
    /// A new session of `kind`, with its id chosen now when the agent
    /// allows that.
    #[must_use]
    pub fn new_session(kind: AgentKind) -> Self {
        let session_id = match kind {
            AgentKind::Claude => Some(uuid::Uuid::new_v4().to_string()),
            AgentKind::Codex => None,
        };
        Self {
            kind,
            session_id,
            history: AgentHistory::Unknown,
        }
    }

    /// The command that starts this session for the first time.
    #[must_use]
    pub fn launch_argv(&self) -> Vec<String> {
        let mut argv = vec![self.kind.program().to_owned()];
        if let (AgentKind::Claude, Some(id)) = (self.kind, &self.session_id) {
            argv.extend(["--session-id".into(), id.clone()]);
        }
        argv
    }

    /// The command that brings this pane back after a restart: resume
    /// its conversation, or start afresh (same id) when its hooks say it
    /// never had one.
    #[must_use]
    pub fn restore_argv(&self) -> Vec<String> {
        if self.history.resumable() {
            self.resume_argv()
        } else {
            self.launch_argv()
        }
    }

    /// The command that resumes this session.
    #[must_use]
    pub fn resume_argv(&self) -> Vec<String> {
        let mut argv = vec![self.kind.program().to_owned()];
        match self.kind {
            AgentKind::Claude => argv.push("--resume".into()),
            AgentKind::Codex => argv.push("resume".into()),
        }
        argv.extend(self.session_id.clone());
        argv
    }

    /// What the restore notice says about this pane.
    #[must_use]
    pub fn resume_note(&self) -> String {
        let name = self.kind.display_name();
        if !self.history.resumable() {
            return format!("starting {name} again (it had no conversation yet)");
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

    #[test]
    fn claude_launches_with_a_chosen_id_and_resumes_it() {
        let agent = PaneAgent::new_session(AgentKind::Claude);
        let id = agent.session_id.clone().expect("chosen at launch");
        assert!(uuid::Uuid::parse_str(&id).is_ok(), "claude requires a UUID");
        assert_eq!(agent.launch_argv(), ["claude", "--session-id", id.as_str()]);
        assert_eq!(agent.resume_argv(), ["claude", "--resume", id.as_str()]);
        assert!(agent.resume_note().contains(&id));
    }

    #[test]
    fn each_claude_session_gets_its_own_id() {
        let a = PaneAgent::new_session(AgentKind::Claude);
        let b = PaneAgent::new_session(AgentKind::Claude);
        assert_ne!(a.session_id, b.session_id);
    }

    #[test]
    fn codex_without_an_id_resumes_through_the_picker() {
        let agent = PaneAgent::new_session(AgentKind::Codex);
        assert_eq!(agent.launch_argv(), ["codex"]);
        assert_eq!(agent.resume_argv(), ["codex", "resume"], "never --last");
        assert!(agent.resume_note().starts_with("pick the Codex session"));
    }

    #[test]
    fn codex_with_a_reported_id_resumes_it() {
        let agent = PaneAgent {
            kind: AgentKind::Codex,
            session_id: Some("0199a213-81c0-7800-8aa1-bbab2a035a53".into()),
            history: AgentHistory::Started,
        };
        assert_eq!(
            agent.resume_argv(),
            ["codex", "resume", "0199a213-81c0-7800-8aa1-bbab2a035a53"]
        );
    }

    #[test]
    fn a_pane_never_prompted_starts_afresh_with_the_same_id() {
        let mut agent = PaneAgent::new_session(AgentKind::Claude);
        let id = agent.session_id.clone().expect("chosen");
        assert_eq!(
            agent.restore_argv(),
            agent.resume_argv(),
            "no hooks: resume"
        );
        agent.history = AgentHistory::Empty;
        assert_eq!(
            agent.restore_argv(),
            ["claude", "--session-id", id.as_str()]
        );
        assert!(agent.resume_note().contains("no conversation yet"));
        let old = r#"{"kind":"claude","session_id":"x"}"#;
        let read: PaneAgent = serde_json::from_str(old).expect("older state file");
        assert_eq!(read.history, AgentHistory::Unknown);
    }
}
