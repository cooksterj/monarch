//! `agent.wait`: has a pane's agent reached one of some states?
//!
//! It answers at once; callers ask again until it says yes (the CLI's
//! `monarch agent wait` asks five times a second). Passing back the
//! `mark` of the first answer catches states that came and went between
//! two questions.

use monarch_core::{AgentState, PaneId};
use serde::{Deserialize, Serialize};

/// The question. The answer is an [`crate::AgentWaitReply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWait {
    /// The pane.
    pub pane: PaneId,
    /// The states waited for.
    pub until: Vec<AgentState>,
    /// The `mark` of an earlier answer: a state entered since then counts
    /// even if the pane has left it again. `None` on the first question,
    /// which only looks at the current state.
    #[serde(default)]
    pub since: Option<u64>,
}

impl AgentWait {
    /// The method name.
    pub const METHOD: &'static str = "agent.wait";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_since_is_optional() {
        let wait = AgentWait {
            pane: PaneId(3),
            until: vec![AgentState::Blocked, AgentState::Done],
            since: Some(7),
        };
        let json = serde_json::to_string(&wait).expect("json");
        assert_eq!(serde_json::from_str::<AgentWait>(&json).ok(), Some(wait));
        let first: AgentWait =
            serde_json::from_str(r#"{"pane":3,"until":["idle"]}"#).expect("json");
        assert_eq!(first.since, None);
        assert!(serde_json::from_str::<AgentWait>(r#"{"pane":3,"until":["busy"]}"#).is_err());
    }
}
