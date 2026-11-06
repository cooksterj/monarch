//! `agent.read`: the last thing a pane's agent said.
//!
//! An agent that started another one waits for it with `agent.wait`,
//! then reads its answer here: the final message of its last turn, as
//! its hooks delivered it. For an agent without hooks, the pane's screen
//! is the best there is.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// The question. The answer is an [`crate::AgentReadReply`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRead {
    /// The pane.
    pub pane: PaneId,
}

impl AgentRead {
    /// The method name.
    pub const METHOD: &'static str = "agent.read";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_needs_a_pane() {
        let read = AgentRead { pane: PaneId(4) };
        let json = serde_json::to_string(&read).expect("json");
        assert_eq!(json, r#"{"pane":4}"#);
        assert_eq!(serde_json::from_str::<AgentRead>(&json).ok(), Some(read));
        assert!(serde_json::from_str::<AgentRead>("{}").is_err());
    }
}
