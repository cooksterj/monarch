//! The last few states each pane's agent entered, so `agent.explain` and
//! the client's agent info can say how it got where it is.

use std::collections::VecDeque;

use monarch_core::{PaneId, UnixMillis};
use monarch_proto::{AgentStatus, AgentTransition};

use super::StateEngine;

/// Transitions kept per pane.
pub(super) const TRANSITIONS: usize = 8;

impl StateEngine {
    /// Notes that `pane`'s status became `status` at `now`, from the
    /// signal that owns it now. A change of activity alone updates the
    /// latest entry; a new state or source adds one, dropping the oldest
    /// past [`TRANSITIONS`]. A pane back to a shell keeps nothing.
    pub fn log(&mut self, pane: PaneId, status: &AgentStatus, now: UnixMillis) {
        let source = self.source(pane);
        if status.state == monarch_core::AgentState::Unknown {
            self.transitions.remove(&pane);
            return;
        }
        let log = self.transitions.entry(pane).or_default();
        if let Some(last) = log
            .back_mut()
            .filter(|t| t.state == status.state && t.source == source)
        {
            last.activity.clone_from(&status.activity);
            return;
        }
        log.push_back(AgentTransition {
            state: status.state,
            source,
            activity: status.activity.clone(),
            at: now,
        });
        while log.len() > TRANSITIONS {
            log.pop_front();
        }
    }

    /// The states `pane`'s agent entered lately, oldest first.
    #[must_use]
    pub fn transitions(&self, pane: PaneId) -> Vec<AgentTransition> {
        self.transitions
            .get(&pane)
            .map(|log| log.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Drops `pane`'s log.
    pub(super) fn forget_transitions(&mut self, pane: PaneId) {
        self.transitions.remove(&pane);
    }
}

/// The type of the log kept per pane.
pub(super) type TransitionLog = VecDeque<AgentTransition>;

#[cfg(test)]
mod tests {
    use monarch_core::{AgentKind, AgentState};
    use monarch_proto::{AgentReport, StatusSource};

    use super::*;

    const P: PaneId = PaneId(1);

    fn status(state: AgentState, activity: Option<&str>) -> AgentStatus {
        AgentStatus {
            state,
            activity: activity.map(Into::into),
            agent: Some(AgentKind::Claude),
            subagents: Vec::new(),
        }
    }

    #[test]
    fn states_are_logged_with_their_source_and_the_oldest_dropped() {
        let mut engine = StateEngine::default();
        assert!(engine.transitions(P).is_empty());
        engine.launched(P, AgentKind::Claude);
        engine.log(P, &status(AgentState::Idle, None), UnixMillis(1_000));
        engine.report(
            P,
            &AgentReport {
                state: Some(AgentState::Working),
                ..AgentReport::new(P, AgentKind::Claude)
            },
        );
        engine.log(
            P,
            &status(AgentState::Working, Some("Edit a.rs")),
            UnixMillis(2_000),
        );
        engine.log(
            P,
            &status(AgentState::Working, Some("Bash ls")),
            UnixMillis(3_000),
        );
        let log = engine.transitions(P);
        assert_eq!(log.len(), 2, "a new activity updates the entry");
        assert_eq!(
            (log[0].state, log[0].source, log[0].at),
            (AgentState::Idle, StatusSource::Launch, UnixMillis(1_000))
        );
        assert_eq!(
            (log[1].source, log[1].activity.as_deref(), log[1].at),
            (StatusSource::Hooks, Some("Bash ls"), UnixMillis(2_000))
        );
        engine.log(P, &AgentStatus::default(), UnixMillis(99_000));
        assert!(engine.transitions(P).is_empty(), "a shell has no history");
    }

    #[test]
    fn only_the_last_few_are_kept_and_an_exited_pane_keeps_none() {
        let mut engine = StateEngine::default();
        for i in 0..20 {
            let state = if i % 2 == 0 {
                AgentState::Blocked
            } else {
                AgentState::Working
            };
            engine.log(P, &status(state, None), UnixMillis(10_000 + i));
        }
        let log = engine.transitions(P);
        assert_eq!(log.len(), TRANSITIONS);
        assert_eq!(log.last().map(|t| t.at), Some(UnixMillis(10_019)));
        engine.forget(P);
        assert!(engine.transitions(P).is_empty());
    }
}
