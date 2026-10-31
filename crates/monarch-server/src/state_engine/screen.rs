//! Statuses read from the screen by an agent's detection manifest, for
//! agents without hooks.
//!
//! A pane whose agent reports through hooks ignores its screen: hooks say
//! more and say it first. Elsewhere the screen's state replaces the
//! current one, except that a turn ending where nobody is looking is
//! Done (it needs attention) until the user sees or answers it, and one
//! ending in front of the user is Finished.

use monarch_core::{AgentKind, AgentState, PaneId};
use monarch_proto::AgentStatus;

use super::StateEngine;

impl StateEngine {
    /// The screen of `pane`, running `agent` (if Monarch knows the kind),
    /// shows `state`; `focused` says whether the user is on that pane.
    /// Returns the new status if it changed.
    pub fn screen(
        &mut self,
        pane: PaneId,
        state: AgentState,
        agent: Option<AgentKind>,
        focused: bool,
    ) -> Option<AgentStatus> {
        if self.hooked.contains(&pane) {
            return None;
        }
        let current = self.status(pane);
        let state = match (current.state, state) {
            (AgentState::Working, AgentState::Idle) if !focused => AgentState::Done,
            (AgentState::Working, AgentState::Idle) => AgentState::Finished,
            (finished @ (AgentState::Done | AgentState::Finished), AgentState::Idle) => finished,
            (_, state) => state,
        };
        let next = AgentStatus {
            state,
            activity: (state == current.state)
                .then(|| current.activity.clone())
                .flatten(),
            agent: agent.or(current.agent),
        };
        self.guessed.insert(pane);
        self.screened.insert(pane);
        self.replace(pane, &current, next)
    }

    /// The agent in `pane` is gone (another program is in front): a
    /// status read from its screen ends, and the pane is a shell again.
    pub fn screen_gone(&mut self, pane: PaneId) -> Option<AgentStatus> {
        if !self.screened.remove(&pane) || self.hooked.contains(&pane) {
            return None;
        }
        self.guessed.remove(&pane);
        let current = self.status(pane);
        self.replace(pane, &current, AgentStatus::default())
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::AgentReport;

    use super::*;

    const P: PaneId = PaneId(1);

    fn state(engine: &StateEngine) -> AgentState {
        engine.status(P).state
    }

    #[test]
    fn a_turn_ending_unseen_is_done_until_seen() {
        let mut engine = StateEngine::default();
        engine.screen(P, AgentState::Working, None, false);
        assert_eq!(state(&engine), AgentState::Working);
        engine.screen(P, AgentState::Idle, None, false);
        assert_eq!(state(&engine), AgentState::Done);
        assert_eq!(
            engine.screen(P, AgentState::Idle, None, false),
            None,
            "stays done"
        );
        let seen = engine.seen(P).expect("changed");
        assert_eq!(seen.state, AgentState::Finished, "an agent, not a shell");
        assert_eq!(engine.screen(P, AgentState::Idle, None, true), None);
        engine.screen(P, AgentState::Working, None, true);
        engine.screen(P, AgentState::Idle, None, true);
        assert_eq!(state(&engine), AgentState::Finished, "watched: finished");
        engine.screen(P, AgentState::Idle, None, false);
        assert_eq!(state(&engine), AgentState::Finished, "seen stays seen");
    }

    #[test]
    fn answering_a_screen_prompt_leaves_the_agent_idle() {
        let mut engine = StateEngine::default();
        let codex = Some(AgentKind::Codex);
        let blocked = engine.screen(P, AgentState::Blocked, codex, false);
        assert_eq!(
            blocked.map(|s| (s.state, s.agent)),
            Some((AgentState::Blocked, codex))
        );
        let idle = engine.answered(P).expect("changed");
        assert_eq!((idle.state, idle.agent), (AgentState::Idle, codex));
        engine.screen(P, AgentState::Blocked, None, false);
        assert_eq!(engine.status(P).agent, codex, "kind kept");
        let gone = engine.screen_gone(P).expect("changed");
        assert_eq!(gone, AgentStatus::default());
        assert_eq!(engine.screen_gone(P), None);
    }

    #[test]
    fn hooks_outrank_the_screen() {
        let mut engine = StateEngine::default();
        let report = AgentReport {
            state: Some(AgentState::Working),
            ..AgentReport::new(P, AgentKind::Claude)
        };
        engine.report(P, &report);
        assert_eq!(engine.screen(P, AgentState::Blocked, None, false), None);
        assert_eq!(engine.screen_gone(P), None);
        assert_eq!(state(&engine), AgentState::Working);
    }
}
