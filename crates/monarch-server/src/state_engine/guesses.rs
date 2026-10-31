//! Statuses guessed from OSC notifications, for agents without hooks.
//!
//! A guess never overrides a hook: once a pane's agent has reported, its
//! notifications are ignored. Nothing reports the end of a guessed
//! Blocked or Done, so the user answering the pane ends it: an agent goes
//! back to Idle, and a plain program back to a shell.

use monarch_core::{AgentKind, AgentState, PaneId};
use monarch_detect::OscSignal;
use monarch_proto::AgentStatus;

use super::StateEngine;

impl StateEngine {
    /// Applies a notification from `pane`, whose foreground program is
    /// `agent` if it is a known one; returns the new status if it changed.
    pub fn signal(
        &mut self,
        pane: PaneId,
        signal: &OscSignal,
        agent: Option<AgentKind>,
    ) -> Option<AgentStatus> {
        if self.hooked.contains(&pane) {
            return None;
        }
        let current = self.status(pane);
        let agent = agent.or(current.agent);
        if agent.is_none() && signal.state == AgentState::Idle {
            // A plain program finished its progress: a shell again.
            self.guessed.remove(&pane);
            return self.replace(pane, &current, AgentStatus::default());
        }
        let next = AgentStatus {
            state: signal.state,
            activity: match &signal.activity {
                Some(text) => AgentStatus::activity_line(text),
                None => current.activity.clone(),
            },
            agent,
        };
        self.guessed.insert(pane);
        self.replace(pane, &current, next)
    }

    /// The user answered `pane` (pressed Enter or typed a reply): a
    /// guessed Blocked, Done or Finished is over. Returns the new status if it
    /// changed.
    pub fn answered(&mut self, pane: PaneId) -> Option<AgentStatus> {
        if !self.guessed.contains(&pane) {
            return None;
        }
        let current = self.status(pane);
        if !matches!(
            current.state,
            AgentState::Blocked | AgentState::Done | AgentState::Finished
        ) {
            return None;
        }
        let next = if current.agent.is_some() || self.screened.contains(&pane) {
            AgentStatus {
                state: AgentState::Idle,
                activity: None,
                agent: current.agent,
            }
        } else {
            self.guessed.remove(&pane);
            AgentStatus::default()
        };
        self.replace(pane, &current, next)
    }

    /// Stores `next` for `pane` if it differs from `current`.
    pub(super) fn replace(
        &mut self,
        pane: PaneId,
        current: &AgentStatus,
        next: AgentStatus,
    ) -> Option<AgentStatus> {
        if next == *current {
            return None;
        }
        if next == AgentStatus::default() {
            self.panes.remove(&pane);
        } else {
            self.panes.insert(pane, next.clone());
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::AgentReport;

    use super::*;

    fn osc(number: u32, payload: &str) -> OscSignal {
        OscSignal::parse(number, payload).expect("a signal")
    }

    #[test]
    fn notifications_set_state_until_answered() {
        let mut engine = StateEngine::default();
        let codex = Some(AgentKind::Codex);
        let blocked = engine.signal(PaneId(1), &osc(9, "Approval requested: rm"), codex);
        assert_eq!(blocked.as_ref().map(|s| s.state), Some(AgentState::Blocked));
        assert_eq!(
            blocked.and_then(|s| s.activity).as_deref(),
            Some("Approval requested: rm")
        );
        let idle = engine.answered(PaneId(1)).expect("changed");
        assert_eq!((idle.state, idle.agent), (AgentState::Idle, codex));
        assert_eq!(engine.answered(PaneId(1)), None, "nothing left to answer");
        let working = engine
            .signal(PaneId(1), &osc(9, "4;3"), None)
            .expect("changed");
        assert_eq!(working.agent, codex, "kind kept");
        assert_eq!(
            engine.answered(PaneId(1)),
            None,
            "working isn't ended by typing"
        );
    }

    #[test]
    fn plain_programs_go_back_to_shells() {
        let mut engine = StateEngine::default();
        engine.signal(PaneId(2), &osc(777, "notify;make;Build finished"), None);
        assert_eq!(engine.status(PaneId(2)).state, AgentState::Done);
        assert_eq!(engine.answered(PaneId(2)), Some(AgentStatus::default()));
        assert_eq!(engine.status(PaneId(2)), AgentStatus::default());
        engine.signal(PaneId(2), &osc(9, "4;1;50"), None);
        assert_eq!(engine.status(PaneId(2)).state, AgentState::Working);
        engine.signal(PaneId(2), &osc(9, "4;0"), None);
        assert_eq!(
            engine.status(PaneId(2)),
            AgentStatus::default(),
            "no Idle shell"
        );
    }

    #[test]
    fn hooks_win_over_notifications() {
        let mut engine = StateEngine::default();
        let working = AgentReport {
            state: Some(AgentState::Working),
            ..AgentReport::new(PaneId(3), AgentKind::Claude)
        };
        engine.report(PaneId(3), &working);
        assert_eq!(engine.signal(PaneId(3), &osc(9, "Done!"), None), None);
        assert_eq!(engine.answered(PaneId(3)), None);
        let ended = AgentReport {
            state: Some(AgentState::Unknown),
            ..working
        };
        engine.report(PaneId(3), &ended);
        assert!(
            engine.signal(PaneId(3), &osc(9, "Done!"), None).is_some(),
            "agent gone: guesses again"
        );
        engine.forget(PaneId(3));
        assert_eq!(engine.answered(PaneId(3)), None);
    }
}
