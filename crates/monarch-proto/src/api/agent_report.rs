//! `agent.report`: what an agent's hook says it is doing.

use monarch_core::{AgentKind, AgentState, PaneId};
use serde::{Deserialize, Serialize};

/// Sent by `monarch hook <agent>` from inside a pane. Hooks are the
/// authoritative signal: a reported state replaces whatever was guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReport {
    /// The pane the agent runs in (`MONARCH_PANE`).
    pub pane: PaneId,
    /// Which agent reports.
    pub agent: AgentKind,
    /// Its new state; `None` leaves the state as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<AgentState>,
    /// A new activity line; `None` keeps the current one, an empty string
    /// clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    /// The agent's own session id, for resuming it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl AgentReport {
    /// The API method name.
    pub const METHOD: &'static str = "agent.report";

    /// A report from `agent` in `pane` that changes nothing yet.
    #[must_use]
    pub fn new(pane: PaneId, agent: AgentKind) -> Self {
        Self {
            pane,
            agent,
            state: None,
            activity: None,
            session_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_set_fields_are_sent() {
        let report = AgentReport {
            state: Some(AgentState::Blocked),
            ..AgentReport::new(PaneId(2), AgentKind::Claude)
        };
        let json = serde_json::to_string(&report).expect("json");
        assert_eq!(json, r#"{"pane":2,"agent":"claude","state":"blocked"}"#);
        let back: AgentReport = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, report);
    }

    #[test]
    fn unknown_agents_are_refused() {
        let line = r#"{"pane":2,"agent":"gemini"}"#;
        assert!(serde_json::from_str::<AgentReport>(line).is_err());
    }
}
