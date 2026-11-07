//! When an agent's state and turn began, and how long its last turn took.

use std::time::Duration;

use monarch_core::UnixMillis;
use serde::{Deserialize, Serialize};

/// The clock facts behind the age shown beside an agent. Clients show a
/// running count for a turn in progress or a wait on the user (from
/// `turn_began` or `since`), and the frozen `took` once a turn has ended.
/// The server and its clients share a machine, so wall clocks agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTiming {
    /// When the agent entered its current state (kept over activity-only
    /// changes and from Done to Finished).
    pub since: UnixMillis,
    /// When the turn in progress began: the first Working after a rest,
    /// kept while it waits on a prompt; `None` between turns.
    pub turn_began: Option<UnixMillis>,
    /// How long the last turn took, from its start to Done, Finished,
    /// Idle or Error; `None` until one has ended.
    pub took: Option<Duration>,
}

impl AgentTiming {
    /// A state that began at `now`, as a turn if `working`.
    #[must_use]
    pub fn begun(now: UnixMillis, working: bool) -> Self {
        Self {
            since: now,
            turn_began: working.then_some(now),
            took: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begins_a_turn_only_when_working() {
        let working = AgentTiming::begun(UnixMillis(5), true);
        assert_eq!(working.turn_began, Some(UnixMillis(5)));
        let blocked = AgentTiming::begun(UnixMillis(5), false);
        assert_eq!(blocked.turn_began, None);
        assert_eq!(blocked.took, None);
        let timing = AgentTiming {
            took: Some(Duration::from_millis(1500)),
            ..working
        };
        let bytes = postcard::to_allocvec(&timing).expect("encode");
        assert_eq!(
            postcard::from_bytes::<AgentTiming>(&bytes).ok(),
            Some(timing)
        );
    }
}
