//! A moment on the wall clock, as the server stamps it and clients age it.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch: when an agent entered its state,
/// sent to clients so they can show how long it has been there. The
/// server and its clients share a machine, so wall clocks agree.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct UnixMillis(pub u64);

impl UnixMillis {
    /// How long ago this moment was at `now`; zero if `now` is earlier.
    #[must_use]
    pub fn age_at(self, now: UnixMillis) -> Duration {
        Duration::from_millis(now.0.saturating_sub(self.0))
    }
}

impl From<SystemTime> for UnixMillis {
    fn from(time: SystemTime) -> Self {
        let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        Self(u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
    }
}

impl From<UnixMillis> for SystemTime {
    fn from(millis: UnixMillis) -> Self {
        UNIX_EPOCH + Duration::from_millis(millis.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_never_go_negative() {
        let then = UnixMillis(10_000);
        assert_eq!(then.age_at(UnixMillis(13_500)), Duration::from_millis(3500));
        assert_eq!(
            then.age_at(UnixMillis(9_000)),
            Duration::ZERO,
            "clock went back"
        );
        assert_eq!(then.age_at(then), Duration::ZERO);
    }

    #[test]
    fn converts_with_system_time_both_ways() {
        let time = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        let millis = UnixMillis::from(time);
        assert_eq!(millis, UnixMillis(1_700_000_000_123));
        assert_eq!(SystemTime::from(millis), time);
        assert_eq!(UnixMillis::from(UNIX_EPOCH), UnixMillis(0));
        let before = UNIX_EPOCH - Duration::from_secs(1);
        assert_eq!(UnixMillis::from(before), UnixMillis(0), "pre-epoch is zero");
        let json = serde_json::to_string(&millis).expect("encode");
        assert_eq!(json, "1700000000123");
    }
}
