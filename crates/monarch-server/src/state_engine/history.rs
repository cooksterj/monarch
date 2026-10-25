//! Which states each pane has entered, and when, so someone waiting for
//! a state can't miss one that came and went between two looks.

use monarch_core::{AgentState, PaneId};

use super::StateEngine;

impl StateEngine {
    /// Notes that `pane` is now in `state`.
    pub fn record(&mut self, pane: PaneId, state: AgentState) {
        self.changes += 1;
        let entered = self.entered.entry(pane).or_default();
        entered.retain(|(s, _)| *s != state);
        entered.push((state, self.changes));
    }

    /// How many status changes there have been: a mark to ask
    /// [`Self::entered_since`] about later.
    #[must_use]
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// Which of `states` `pane` has entered after the mark `since`, if
    /// any (the latest).
    #[must_use]
    pub fn entered_since(
        &self,
        pane: PaneId,
        states: &[AgentState],
        since: u64,
    ) -> Option<AgentState> {
        self.entered
            .get(&pane)?
            .iter()
            .filter(|(s, at)| *at > since && states.contains(s))
            .max_by_key(|(_, at)| *at)
            .map(|(s, _)| *s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_entered_after_a_mark_are_remembered() {
        let mut engine = StateEngine::default();
        let p = PaneId(1);
        engine.record(p, AgentState::Working);
        let mark = engine.changes();
        assert_eq!(
            engine.entered_since(p, &[AgentState::Working], mark),
            None,
            "before"
        );
        engine.record(p, AgentState::Blocked);
        engine.record(p, AgentState::Working);
        let wanted = [AgentState::Blocked, AgentState::Done];
        assert_eq!(
            engine.entered_since(p, &wanted, mark),
            Some(AgentState::Blocked),
            "came and went"
        );
        engine.record(p, AgentState::Done);
        assert_eq!(
            engine.entered_since(p, &wanted, mark),
            Some(AgentState::Done),
            "the latest"
        );
        assert_eq!(engine.entered_since(PaneId(2), &wanted, 0), None);
        engine.forget(p);
        assert_eq!(engine.entered_since(p, &wanted, 0), None);
    }
}
