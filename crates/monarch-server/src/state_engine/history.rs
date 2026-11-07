//! Which states each pane has entered, and when, so someone waiting for
//! a state can't miss one that came and went between two looks, and the
//! sidebar can say how long an agent has been where it is.

use monarch_core::{AgentState, PaneId, UnixMillis};

use super::StateEngine;

impl StateEngine {
    /// Notes that `pane` is now in `state`, at `now`. Returns when the
    /// state began: `now` if it is new, else when it was first entered
    /// (a change of activity keeps it, and so does Done becoming
    /// Finished, since the agent finished at the same moment); `None`
    /// for a plain shell.
    pub fn record(
        &mut self,
        pane: PaneId,
        state: AgentState,
        now: UnixMillis,
    ) -> Option<UnixMillis> {
        self.changes += 1;
        let entered = self.entered.entry(pane).or_default();
        entered.retain(|(s, _)| *s != state);
        entered.push((state, self.changes));
        if state == AgentState::Unknown {
            self.since.remove(&pane);
            return None;
        }
        let kept = match self.since.get(&pane) {
            Some((was, began)) if *was == state => Some(*began),
            Some((AgentState::Done, began)) if state == AgentState::Finished => Some(*began),
            _ => None,
        };
        let began = kept.unwrap_or(now);
        self.since.insert(pane, (state, began));
        Some(began)
    }

    /// When `pane`'s agent entered its current state; `None` for a plain
    /// shell.
    #[must_use]
    pub fn since(&self, pane: PaneId) -> Option<UnixMillis> {
        self.since.get(&pane).map(|(_, began)| *began)
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

    const T0: UnixMillis = UnixMillis(1_000);

    #[test]
    fn states_entered_after_a_mark_are_remembered() {
        let mut engine = StateEngine::default();
        let p = PaneId(1);
        engine.record(p, AgentState::Working, T0);
        let mark = engine.changes();
        assert_eq!(
            engine.entered_since(p, &[AgentState::Working], mark),
            None,
            "before"
        );
        engine.record(p, AgentState::Blocked, T0);
        engine.record(p, AgentState::Working, T0);
        let wanted = [AgentState::Blocked, AgentState::Done];
        assert_eq!(
            engine.entered_since(p, &wanted, mark),
            Some(AgentState::Blocked),
            "came and went"
        );
        engine.record(p, AgentState::Done, T0);
        assert_eq!(
            engine.entered_since(p, &wanted, mark),
            Some(AgentState::Done),
            "the latest"
        );
        assert_eq!(engine.entered_since(PaneId(2), &wanted, 0), None);
        engine.forget(p);
        assert_eq!(engine.entered_since(p, &wanted, 0), None);
    }

    #[test]
    fn a_state_keeps_its_start_until_it_changes() {
        let mut engine = StateEngine::default();
        let p = PaneId(1);
        assert_eq!(engine.since(p), None, "nothing yet");
        assert_eq!(engine.record(p, AgentState::Working, T0), Some(T0));
        let later = UnixMillis(5_000);
        assert_eq!(
            engine.record(p, AgentState::Working, later),
            Some(T0),
            "a new activity in the same state keeps the start"
        );
        assert_eq!(engine.record(p, AgentState::Done, later), Some(later));
        let seen = UnixMillis(9_000);
        assert_eq!(
            engine.record(p, AgentState::Finished, seen),
            Some(later),
            "looking at a Done agent doesn't restart its clock"
        );
        assert_eq!(engine.since(p), Some(later));
        assert_eq!(
            engine.record(p, AgentState::Unknown, seen),
            None,
            "a shell has no state to time"
        );
        assert_eq!(engine.since(p), None);
        assert_eq!(engine.record(p, AgentState::Finished, seen), Some(seen));
        engine.forget(p);
        assert_eq!(engine.since(p), None);
    }
}
