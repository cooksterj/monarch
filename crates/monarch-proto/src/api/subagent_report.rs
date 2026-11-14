//! What a hook says about one sub-agent of the agent it reports for.

use monarch_core::AgentState;
use serde::{Deserialize, Serialize};

/// An event from inside a sub-agent (Claude Code's `agent_id`): one the
/// agent started with its Agent tool, or an in-process teammate. Its
/// [`AgentReport`](crate::AgentReport) still carries the parent's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentReport {
    /// The agent's id for it, unique within the session.
    pub id: String,
    /// What kind it is (`Explore`, `general-purpose`, `my-plugin:reviewer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Its new state; `None` leaves it as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<AgentState>,
    /// A new activity line; `None` keeps it, an empty string clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_set_fields_are_sent() {
        let report = SubagentReport {
            id: "a1".into(),
            kind: Some("Explore".into()),
            state: Some(AgentState::Working),
            activity: None,
        };
        let json = serde_json::to_string(&report).expect("json");
        assert_eq!(json, r#"{"id":"a1","kind":"Explore","state":"working"}"#);
        assert_eq!(
            serde_json::from_str::<SubagentReport>(&json).ok(),
            Some(report)
        );
    }

    #[test]
    fn an_id_is_required() {
        assert!(serde_json::from_str::<SubagentReport>(r#"{"kind":"Plan"}"#).is_err());
    }
}
