//! Keeps the ages shown beside agents (`3m` in the sidebar and inbox)
//! moving: once a second, while any of them is a running count.

use std::time::{Duration, Instant};

use monarch_core::{Clock, SystemClock, UnixMillis};

use super::AttachApp;

/// How often the ages are redrawn.
pub const AGE_STEP: Duration = Duration::from_secs(1);

impl AttachApp {
    /// The time now, for ageing agent states.
    pub(super) fn now() -> UnixMillis {
        SystemClock.now()
    }

    /// Redraws the ages when a second has passed and someone is showing
    /// them: the sidebar, or an open inbox.
    pub(super) fn tick_ages(&mut self) {
        let now = Instant::now();
        if now < self.next_age_tick {
            return;
        }
        self.next_age_tick = now + AGE_STEP;
        let inbox = self
            .popups
            .picker
            .as_ref()
            .is_some_and(crate::Picker::is_list);
        if !self.state.any_running() || (self.state.left() == 0 && !inbox) {
            return;
        }
        self.refresh_inbox();
        self.dirty = true;
    }
}
