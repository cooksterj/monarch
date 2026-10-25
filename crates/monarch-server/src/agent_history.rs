//! Whether an agent pane has a conversation to resume, as its hooks tell.

use monarch_core::AgentState;
use serde::{Deserialize, Serialize};

/// What the hooks have said about a pane's conversation. Resuming one that
/// was never prompted fails (Claude Code saves nothing until the first
/// prompt), so such a pane starts afresh instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHistory {
    /// No hook has reported: resume, as without hooks there is no telling.
    #[default]
    Unknown,
    /// Hooks report, but no prompt has been sent yet.
    Empty,
    /// A prompt was sent: there is a conversation to resume.
    Started,
}

impl AgentHistory {
    /// After a report saying `state` (`None`: no state change). A prompt
    /// makes the agent work, and only a conversation can block or finish.
    #[must_use]
    pub fn after(self, state: Option<AgentState>) -> Self {
        let talked = matches!(
            state,
            Some(AgentState::Working | AgentState::Blocked | AgentState::Done)
        );
        match self {
            Self::Started => Self::Started,
            _ if talked => Self::Started,
            _ => Self::Empty,
        }
    }

    /// Whether restoring should resume rather than start afresh.
    #[must_use]
    pub fn resumable(self) -> bool {
        self != Self::Empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_starts_the_conversation_for_good() {
        let start = AgentHistory::Unknown.after(Some(AgentState::Idle));
        assert_eq!(start, AgentHistory::Empty);
        assert!(!start.resumable() && AgentHistory::Unknown.resumable());
        let talked = start.after(Some(AgentState::Working));
        assert_eq!(talked, AgentHistory::Started);
        assert_eq!(talked.after(Some(AgentState::Idle)), AgentHistory::Started);
        assert_eq!(AgentHistory::Unknown.after(None), AgentHistory::Empty);
    }

    #[test]
    fn reads_and_writes_as_a_word() {
        let json = serde_json::to_string(&AgentHistory::Started).expect("json");
        assert_eq!(json, r#""started""#);
    }
}
