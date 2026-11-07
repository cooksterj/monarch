//! The real wall clock.

use std::time::SystemTime;

use crate::{Clock, UnixMillis};

/// The OS wall clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UnixMillis {
        UnixMillis::from(SystemTime::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_time_after_the_project_began() {
        let now = SystemClock.now();
        assert!(now > UnixMillis(1_760_000_000_000), "{now:?}");
        assert!(SystemClock.now() >= now, "does not run backwards at once");
    }
}
