//! The answer to `agent.wait`.

use monarch_core::AgentState;
use serde::{Deserialize, Serialize};

use crate::AgentStatus;

/// Whether the pane reached a wanted state, and how it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWaitReply {
    /// The wanted state it is in, or entered since the question's
    /// `since`; `None` while still waiting.
    pub reached: Option<AgentState>,
    /// Its status now.
    pub status: AgentStatus,
    /// Pass this back as `since` on the next question.
    pub mark: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let reply = AgentWaitReply {
            reached: Some(AgentState::Done),
            status: AgentStatus::default(),
            mark: 12,
        };
        let json = serde_json::to_string(&reply).expect("json");
        assert_eq!(
            serde_json::from_str::<AgentWaitReply>(&json).ok(),
            Some(reply)
        );
        assert!(serde_json::from_str::<AgentWaitReply>(r#"{"mark":1}"#).is_err());
    }
}
