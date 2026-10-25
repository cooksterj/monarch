//! Each pane's agent state, fused from what its agent reports.
//!
//! Hooks are the only source so far, and they are authoritative: a
//! reported state replaces the current one. OSC notifications, screen
//! manifests and quiescence join later as weaker signals.

use std::collections::BTreeMap;

use monarch_core::{AgentState, PaneId};
use monarch_proto::{AgentReport, AgentStatus};

/// Agent status per pane; panes never reported on are plain shells.
#[derive(Debug, Clone, Default)]
pub struct StateEngine {
    panes: BTreeMap<PaneId, AgentStatus>,
}

impl StateEngine {
    /// What `pane`'s agent is doing (`Unknown` if nothing reported).
    #[must_use]
    pub fn status(&self, pane: PaneId) -> AgentStatus {
        self.panes.get(&pane).cloned().unwrap_or_default()
    }

    /// Applies a hook report; returns the new status if it changed.
    pub fn report(&mut self, report: &AgentReport) -> Option<AgentStatus> {
        let current = self.panes.entry(report.pane).or_default();
        let mut next = current.clone();
        if let Some(state) = report.state {
            next.state = state;
        }
        // An agent that ended leaves a plain shell behind.
        next.agent = (next.state != AgentState::Unknown).then_some(report.agent);
        if let Some(text) = &report.activity {
            next.activity = AgentStatus::activity_line(text);
        }
        if next == *current {
            return None;
        }
        current.clone_from(&next);
        Some(next)
    }

    /// Drops a pane that exited.
    pub fn forget(&mut self, pane: PaneId) {
        self.panes.remove(&pane);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentKind;

    use super::*;

    fn report(state: Option<AgentState>, activity: Option<&str>) -> AgentReport {
        AgentReport {
            state,
            activity: activity.map(Into::into),
            ..AgentReport::new(PaneId(1), AgentKind::Claude)
        }
    }

    #[test]
    fn reports_replace_the_state_and_keep_unsent_fields() {
        let mut engine = StateEngine::default();
        assert_eq!(engine.status(PaneId(1)), AgentStatus::default());
        let working = engine.report(&report(Some(AgentState::Working), Some("Edit a.rs")));
        assert_eq!(
            working,
            Some(AgentStatus {
                state: AgentState::Working,
                activity: Some("Edit a.rs".into()),
                agent: Some(AgentKind::Claude),
            })
        );
        let blocked = engine.report(&report(Some(AgentState::Blocked), None));
        assert_eq!(
            blocked.and_then(|s| s.activity).as_deref(),
            Some("Edit a.rs")
        );
        assert_eq!(engine.status(PaneId(1)).state, AgentState::Blocked);
    }

    #[test]
    fn unchanged_reports_publish_nothing_and_blank_activity_clears() {
        let mut engine = StateEngine::default();
        engine.report(&report(Some(AgentState::Done), Some("All done\nmore")));
        assert_eq!(
            engine.report(&report(Some(AgentState::Done), Some("All done"))),
            None
        );
        let ended = engine.report(&report(Some(AgentState::Unknown), None));
        assert_eq!(ended.map(|s| s.agent), Some(None), "back to a shell");
        let cleared = engine.report(&report(None, Some("")));
        assert_eq!(cleared.map(|s| s.activity), Some(None));
        engine.forget(PaneId(1));
        assert_eq!(engine.status(PaneId(1)), AgentStatus::default());
    }
}
