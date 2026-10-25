//! Each pane's agent state, fused from what its agent reports.
//!
//! Hooks are authoritative: a reported state replaces the current one,
//! and once a pane's agent has reported, weaker signals are ignored there.
//! OSC notifications (see `guesses`) and screen manifests (see `screen`)
//! are the fallback for agents without hooks.

mod guesses;
mod screen;

use std::collections::{BTreeMap, BTreeSet};

use monarch_core::{AgentKind, AgentState, PaneId};
use monarch_proto::{AgentReport, AgentStatus, StatusSource};

/// Agent status per pane (panes never reported on are plain shells),
/// which pane each agent session id was seen in, and where each status
/// came from.
#[derive(Debug, Clone, Default)]
pub struct StateEngine {
    panes: BTreeMap<PaneId, AgentStatus>,
    sessions: BTreeMap<String, PaneId>,
    /// Panes whose agent reports through hooks.
    hooked: BTreeSet<PaneId>,
    /// Panes whose status was guessed from a weaker signal.
    guessed: BTreeSet<PaneId>,
    /// Panes running an agent known by its screen manifest.
    screened: BTreeSet<PaneId>,
}

impl StateEngine {
    /// What `pane`'s agent is doing (`Unknown` if nothing reported).
    #[must_use]
    pub fn status(&self, pane: PaneId) -> AgentStatus {
        self.panes.get(&pane).cloned().unwrap_or_default()
    }

    /// The pane an agent session was last reported from.
    #[must_use]
    pub fn pane_for(&self, session_id: &str) -> Option<PaneId> {
        self.sessions.get(session_id).copied()
    }

    /// Whether some agent session is known to run in `pane`.
    #[must_use]
    pub fn has_session(&self, pane: PaneId) -> bool {
        self.sessions.values().any(|p| *p == pane)
    }

    /// Applies a hook report about `pane`; returns the new status if it
    /// changed. A session id in the report now belongs to `pane` alone.
    pub fn report(&mut self, pane: PaneId, report: &AgentReport) -> Option<AgentStatus> {
        if let Some(id) = &report.session_id {
            self.sessions.retain(|_, p| *p != pane);
            self.sessions.insert(id.clone(), pane);
        }
        self.guessed.remove(&pane);
        let current = self.panes.entry(pane).or_default();
        let mut next = current.clone();
        if let Some(state) = report.state {
            next.state = state;
        }
        // An agent that ended leaves a plain shell behind.
        next.agent = (next.state != AgentState::Unknown).then_some(report.agent);
        if next.agent.is_some() {
            self.hooked.insert(pane);
        } else {
            self.hooked.remove(&pane);
        }
        if let Some(text) = &report.activity {
            next.activity = AgentStatus::activity_line(text);
        }
        if next == *current {
            return None;
        }
        current.clone_from(&next);
        Some(next)
    }

    /// Monarch started `kind` in `pane`: idle, labelled as that agent,
    /// until something reports on it.
    pub fn launched(&mut self, pane: PaneId, kind: AgentKind) -> Option<AgentStatus> {
        let current = self.status(pane);
        let next = AgentStatus {
            state: AgentState::Idle,
            activity: None,
            agent: Some(kind),
        };
        self.replace(pane, &current, next)
    }

    /// Which signal `pane`'s status came from.
    #[must_use]
    pub fn source(&self, pane: PaneId) -> StatusSource {
        if self.hooked.contains(&pane) {
            StatusSource::Hooks
        } else if self.screened.contains(&pane) {
            StatusSource::Screen
        } else if self.guessed.contains(&pane) {
            StatusSource::Notification
        } else if self.panes.contains_key(&pane) {
            StatusSource::Launch
        } else {
            StatusSource::None
        }
    }

    /// The user looked at `pane`: a Done agent is now Idle (keeping its
    /// summary), and a plain program that finished is a shell again.
    /// Returns the new status if it changed.
    pub fn seen(&mut self, pane: PaneId) -> Option<AgentStatus> {
        let status = self.panes.get_mut(&pane)?;
        if status.state != AgentState::Done {
            return None;
        }
        if status.agent.is_none() && !self.screened.contains(&pane) {
            self.panes.remove(&pane);
            self.guessed.remove(&pane);
            return Some(AgentStatus::default());
        }
        status.state = AgentState::Idle;
        Some(status.clone())
    }

    /// Drops a pane that exited.
    pub fn forget(&mut self, pane: PaneId) {
        self.panes.remove(&pane);
        self.sessions.retain(|_, p| *p != pane);
        self.hooked.remove(&pane);
        self.guessed.remove(&pane);
        self.screened.remove(&pane);
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
        let working = engine.report(
            PaneId(1),
            &report(Some(AgentState::Working), Some("Edit a.rs")),
        );
        assert_eq!(
            working,
            Some(AgentStatus {
                state: AgentState::Working,
                activity: Some("Edit a.rs".into()),
                agent: Some(AgentKind::Claude),
            })
        );
        let blocked = engine.report(PaneId(1), &report(Some(AgentState::Blocked), None));
        assert_eq!(
            blocked.and_then(|s| s.activity).as_deref(),
            Some("Edit a.rs")
        );
        assert_eq!(engine.status(PaneId(1)).state, AgentState::Blocked);
    }

    #[test]
    fn unchanged_reports_publish_nothing_and_blank_activity_clears() {
        let mut engine = StateEngine::default();
        engine.report(
            PaneId(1),
            &report(Some(AgentState::Done), Some("All done\nmore")),
        );
        assert_eq!(
            engine.report(PaneId(1), &report(Some(AgentState::Done), Some("All done"))),
            None
        );
        let ended = engine.report(PaneId(1), &report(Some(AgentState::Unknown), None));
        assert_eq!(ended.map(|s| s.agent), Some(None), "back to a shell");
        let cleared = engine.report(PaneId(1), &report(None, Some("")));
        assert_eq!(cleared.map(|s| s.activity), Some(None));
        engine.forget(PaneId(1));
        assert_eq!(engine.status(PaneId(1)), AgentStatus::default());
    }

    #[test]
    fn seen_done_becomes_idle_with_its_summary() {
        let mut engine = StateEngine::default();
        engine.report(
            PaneId(1),
            &report(Some(AgentState::Done), Some("Added tests")),
        );
        let idle = engine.seen(PaneId(1)).expect("changed");
        assert_eq!(
            (idle.state, idle.activity.as_deref()),
            (AgentState::Idle, Some("Added tests"))
        );
        assert_eq!(engine.seen(PaneId(1)), None, "only Done decays");
        engine.report(PaneId(1), &report(Some(AgentState::Blocked), None));
        assert_eq!(
            engine.seen(PaneId(1)),
            None,
            "Blocked still needs an answer"
        );
        assert_eq!(engine.seen(PaneId(9)), None);
    }

    #[test]
    fn launched_agents_wait_for_a_signal_and_sources_are_named() {
        let mut engine = StateEngine::default();
        let p = PaneId(1);
        assert_eq!(engine.source(p), StatusSource::None);
        let idle = engine.launched(p, AgentKind::Codex).expect("changed");
        assert_eq!(
            (idle.state, idle.agent),
            (AgentState::Idle, Some(AgentKind::Codex))
        );
        assert_eq!(engine.source(p), StatusSource::Launch);
        engine.screen(p, AgentState::Working, None, false);
        assert_eq!(
            engine.source(p),
            StatusSource::Screen,
            "not mistaken for hooks"
        );
        engine.report(p, &report(Some(AgentState::Working), None));
        assert_eq!(engine.source(p), StatusSource::Hooks);
        engine.forget(p);
        assert_eq!(engine.source(p), StatusSource::None);
    }

    #[test]
    fn session_ids_follow_their_pane() {
        let mut engine = StateEngine::default();
        let with_id = |id: &str| AgentReport {
            session_id: Some(id.into()),
            ..AgentReport::unplaced(AgentKind::Codex)
        };
        engine.report(PaneId(2), &with_id("a"));
        assert_eq!(engine.pane_for("a"), Some(PaneId(2)));
        assert!(engine.has_session(PaneId(2)) && !engine.has_session(PaneId(3)));
        engine.report(PaneId(2), &with_id("b"));
        assert_eq!(
            (engine.pane_for("a"), engine.pane_for("b")),
            (None, Some(PaneId(2))),
            "one session per pane"
        );
        engine.forget(PaneId(2));
        assert_eq!(engine.pane_for("b"), None);
    }
}
