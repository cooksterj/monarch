//! Which states each pane has entered, and when, so someone waiting for
//! a state can't miss one that came and went between two looks, and the
//! sidebar can say how long an agent has been where it is.

use monarch_core::{AgentState, PaneId, UnixMillis};
use monarch_proto::AgentTiming;

use super::StateEngine;

impl StateEngine {
    /// Notes that `pane` is now in `state`, at `now`, and returns the
    /// clock facts for it: when the state began (`now` if it is new; a
    /// change of activity keeps it, and so does Done becoming Finished,
    /// since the agent finished at the same moment), when the turn in
    /// progress began, and how long the last one took. `None` for a plain
    /// shell.
    pub fn record(
        &mut self,
        pane: PaneId,
        state: AgentState,
        now: UnixMillis,
    ) -> Option<AgentTiming> {
        self.changes += 1;
        let entered = self.entered.entry(pane).or_default();
        entered.retain(|(s, _)| *s != state);
        entered.push((state, self.changes));
        if state == AgentState::Unknown {
            self.timing.remove(&pane);
            return None;
        }
        let timing = match self.timing.get(&pane) {
            Some((was, timing)) if *was == state => *timing,
            Some((AgentState::Done, timing)) if state == AgentState::Finished => *timing,
            Some((was, timing)) => advance(*was, *timing, state, now),
            None => AgentTiming::begun(now, state == AgentState::Working),
        };
        self.timing.insert(pane, (state, timing));
        Some(timing)
    }

    /// The clock facts behind `pane`'s age; `None` for a plain shell.
    #[must_use]
    pub fn timing(&self, pane: PaneId) -> Option<AgentTiming> {
        self.timing.get(&pane).map(|(_, timing)| *timing)
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

/// The timing after `was` becomes `state` at `now`: a turn starts with
/// Working (unless it resumes after a prompt), survives Blocked, and ends,
/// measured, with anything else.
fn advance(
    was: AgentState,
    timing: AgentTiming,
    state: AgentState,
    now: UnixMillis,
) -> AgentTiming {
    let mut next = AgentTiming {
        since: now,
        ..timing
    };
    match state {
        AgentState::Working => {
            if was != AgentState::Blocked || next.turn_began.is_none() {
                next.turn_began = Some(now);
            }
        }
        AgentState::Blocked => {}
        _ => {
            if let Some(began) = next.turn_began.take() {
                next.took = Some(began.age_at(now));
            }
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
        assert_eq!(engine.timing(p), None, "nothing yet");
        let t0 = engine.record(p, AgentState::Working, T0).expect("timing");
        assert_eq!((t0.since, t0.turn_began, t0.took), (T0, Some(T0), None));
        let later = UnixMillis(5_000);
        assert_eq!(
            engine.record(p, AgentState::Working, later),
            Some(t0),
            "a new activity in the same state keeps the start"
        );
        let done = engine.record(p, AgentState::Done, later).expect("timing");
        assert_eq!(done.since, later);
        assert_eq!(done.turn_began, None, "the turn is over");
        assert_eq!(done.took, Some(Duration::from_secs(4)));
        let seen = UnixMillis(9_000);
        assert_eq!(
            engine.record(p, AgentState::Finished, seen),
            Some(done),
            "looking at a Done agent doesn't restart its clock"
        );
        assert_eq!(engine.timing(p), Some(done));
        assert_eq!(
            engine.record(p, AgentState::Unknown, seen),
            None,
            "a shell has no state to time"
        );
        assert_eq!(engine.timing(p), None);
        let fresh = engine.record(p, AgentState::Idle, seen).expect("timing");
        assert_eq!((fresh.since, fresh.took), (seen, None), "no turn behind it");
        engine.forget(p);
        assert_eq!(engine.timing(p), None);
    }

    #[test]
    fn a_turn_survives_a_prompt_and_ends_with_rest_or_failure() {
        let mut engine = StateEngine::default();
        let p = PaneId(1);
        engine.record(p, AgentState::Idle, T0);
        engine.record(p, AgentState::Working, UnixMillis(2_000));
        let blocked = engine
            .record(p, AgentState::Blocked, UnixMillis(3_000))
            .expect("timing");
        assert_eq!(blocked.since, UnixMillis(3_000), "the wait starts now");
        assert_eq!(
            blocked.turn_began,
            Some(UnixMillis(2_000)),
            "the turn goes on"
        );
        let resumed = engine
            .record(p, AgentState::Working, UnixMillis(8_000))
            .expect("timing");
        assert_eq!(
            resumed.turn_began,
            Some(UnixMillis(2_000)),
            "still that turn"
        );
        let failed = engine
            .record(p, AgentState::Error, UnixMillis(9_000))
            .expect("timing");
        assert_eq!(
            failed.took,
            Some(Duration::from_secs(7)),
            "prompt time included"
        );
        assert_eq!(failed.turn_began, None);
    }

    #[test]
    fn a_new_turn_keeps_the_last_length_and_ends_when_answered_away() {
        let mut engine = StateEngine::default();
        let p = PaneId(1);
        engine.record(p, AgentState::Working, T0);
        engine.record(p, AgentState::Error, UnixMillis(8_000));
        let again = engine
            .record(p, AgentState::Working, UnixMillis(20_000))
            .expect("timing");
        assert_eq!(again.turn_began, Some(UnixMillis(20_000)), "a new turn");
        assert_eq!(
            again.took,
            Some(Duration::from_secs(7)),
            "the last one is kept"
        );
        let asked = engine
            .record(p, AgentState::Blocked, UnixMillis(21_000))
            .expect("timing");
        let idle = engine
            .record(p, AgentState::Idle, UnixMillis(25_000))
            .expect("timing");
        assert_eq!(
            idle.took,
            Some(Duration::from_secs(5)),
            "answered away: the turn ends"
        );
        assert_eq!(asked.turn_began, Some(UnixMillis(20_000)));
        let question = StateEngine::default()
            .record(p, AgentState::Blocked, T0)
            .expect("timing");
        assert_eq!(question.turn_began, None, "a prompt with no turn behind it");
    }
}
