//! When an agent's state began and how long its turn has worked.

use std::time::Duration;

use monarch_core::UnixMillis;
use serde::{Deserialize, Serialize};

/// The clock facts behind the age shown beside an agent: a stopwatch of
/// the turn's working time, which runs while Working, pauses while the
/// agent waits on the user and stops when the turn ends, plus when the
/// current state began so the inbox can say how long a prompt has waited.
/// The server and its clients share a machine, so wall clocks agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTiming {
    /// When the agent entered its current state (kept over activity-only
    /// changes and from Done to Finished).
    pub since: UnixMillis,
    /// When the current working span began; `None` while the stopwatch
    /// is paused or stopped.
    pub working_since: Option<UnixMillis>,
    /// The turn's working time before the current span: the whole turn
    /// once it has ended; `None` until a turn has started.
    pub worked: Option<Duration>,
}

impl AgentTiming {
    /// A state that began at `now`, starting a turn if `working`.
    #[must_use]
    pub fn begun(now: UnixMillis, working: bool) -> Self {
        Self {
            since: now,
            working_since: working.then_some(now),
            worked: working.then_some(Duration::ZERO),
        }
    }

    /// The turn's working time at `now`: what was worked before the
    /// current span plus the span so far; `None` with no turn started.
    #[must_use]
    pub fn worked_at(&self, now: UnixMillis) -> Option<Duration> {
        let span = self.working_since.map_or(Duration::ZERO, |s| s.age_at(now));
        Some(self.worked? + span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begins_a_turn_only_when_working() {
        let working = AgentTiming::begun(UnixMillis(5_000), true);
        assert_eq!(working.working_since, Some(UnixMillis(5_000)));
        assert_eq!(
            working.worked_at(UnixMillis(7_000)),
            Some(Duration::from_secs(2))
        );
        let blocked = AgentTiming::begun(UnixMillis(5_000), false);
        assert_eq!(blocked.working_since, None);
        assert_eq!(blocked.worked_at(UnixMillis(7_000)), None, "no turn");
        let paused = AgentTiming {
            since: UnixMillis(6_000),
            working_since: None,
            worked: Some(Duration::from_millis(1500)),
        };
        assert_eq!(
            paused.worked_at(UnixMillis(9_000)),
            Some(Duration::from_millis(1500)),
            "stands still"
        );
        let bytes = postcard::to_allocvec(&paused).expect("encode");
        assert_eq!(
            postcard::from_bytes::<AgentTiming>(&bytes).ok(),
            Some(paused)
        );
    }
}
