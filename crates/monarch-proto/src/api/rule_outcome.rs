//! One manifest rule's outcome on a pane's screen, for `agent.explain`.

use monarch_core::AgentState;
use serde::{Deserialize, Serialize};

/// A rule, whether it matched, and the start of the text it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleOutcome {
    /// The rule's id.
    pub id: String,
    /// The state it detects.
    pub state: AgentState,
    /// Its priority (higher wins; the earlier rule on a tie).
    pub priority: i32,
    /// The region it reads (`bottom_non_empty_lines(8)`).
    pub region: String,
    /// Whether its gate held.
    pub matched: bool,
    /// Whether a match keeps the current state instead of setting one.
    pub hold: bool,
    /// The end of the region's text (`…` first when cut).
    pub preview: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let outcome = RuleOutcome {
            id: "permission_prompt".into(),
            state: AgentState::Blocked,
            priority: 900,
            region: "whole_recent".into(),
            matched: true,
            hold: false,
            preview: "Allow execution?".into(),
        };
        let json = serde_json::to_string(&outcome).expect("json");
        assert_eq!(
            serde_json::from_str::<RuleOutcome>(&json).ok(),
            Some(outcome)
        );
        assert!(serde_json::from_str::<RuleOutcome>(r#"{"id":"x"}"#).is_err());
    }
}
