//! What `agent.explain` says about one pane.

use monarch_core::{AgentState, PaneId};
use serde::{Deserialize, Serialize};

use crate::{AgentStatus, RuleOutcome, StatusSource};

/// A pane's status, where it came from, and what its screen shows to
/// its agent's detection manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneExplanation {
    /// The pane.
    pub pane: PaneId,
    /// Its tab, counted from 1.
    pub tab: u32,
    /// Its foreground program.
    pub program: Option<String>,
    /// The status Monarch shows.
    pub status: AgentStatus,
    /// Where that status came from.
    pub source: StatusSource,
    /// The manifest for the program (`gemini 2026.10.1`), if there is one.
    pub manifest: Option<String>,
    /// What the screen shows now: `None` without a manifest, or when the
    /// winning rule keeps the current state.
    pub screen_state: Option<AgentState>,
    /// The winning rule; `None` when nothing matched (which reads idle).
    pub winner: Option<String>,
    /// Every rule, in manifest order.
    pub rules: Vec<RuleOutcome>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let explained = PaneExplanation {
            pane: PaneId(2),
            tab: 1,
            program: Some("zsh".into()),
            status: AgentStatus::default(),
            source: StatusSource::None,
            manifest: None,
            screen_state: None,
            winner: None,
            rules: Vec::new(),
        };
        let json = serde_json::to_string(&explained).expect("json");
        assert_eq!(
            serde_json::from_str::<PaneExplanation>(&json).ok(),
            Some(explained)
        );
        assert!(serde_json::from_str::<PaneExplanation>(r#"{"pane":2}"#).is_err());
    }
}
