//! `agent.report`: what an agent's hook says it is doing.

use monarch_core::{AgentKind, AgentState, PaneId};
use serde::{Deserialize, Serialize};

/// Sent by `monarch hook <agent>`. Hooks are the authoritative signal: a
/// reported state replaces whatever was guessed.
///
/// A hook that runs inside its pane names it (`MONARCH_PANE`). Codex runs
/// hooks in a shared background process whose environment belongs to
/// whichever pane started it, so its reports name no pane; the server
/// finds it from `session_id` (once seen) or `cwd` (a pane running that
/// agent there).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReport {
    /// The pane the agent runs in, when the hook knows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<PaneId>,
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
    /// The agent's working directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl AgentReport {
    /// The API method name.
    pub const METHOD: &'static str = "agent.report";

    /// A report from `agent` in `pane` that changes nothing yet.
    #[must_use]
    pub fn new(pane: PaneId, agent: AgentKind) -> Self {
        Self {
            pane: Some(pane),
            ..Self::unplaced(agent)
        }
    }

    /// A report from `agent` that does not know its pane.
    #[must_use]
    pub fn unplaced(agent: AgentKind) -> Self {
        Self {
            pane: None,
            agent,
            state: None,
            activity: None,
            session_id: None,
            cwd: None,
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
    fn reports_without_a_pane_carry_where_the_agent_works() {
        let report = AgentReport {
            cwd: Some("/w".into()),
            ..AgentReport::unplaced(AgentKind::Codex)
        };
        let json = serde_json::to_string(&report).expect("json");
        assert_eq!(json, r#"{"agent":"codex","cwd":"/w"}"#);
        assert_eq!(
            serde_json::from_str::<AgentReport>(&json).ok(),
            Some(report)
        );
    }

    #[test]
    fn unknown_agents_are_refused() {
        let line = r#"{"pane":2,"agent":"gemini"}"#;
        assert!(serde_json::from_str::<AgentReport>(line).is_err());
    }
}
