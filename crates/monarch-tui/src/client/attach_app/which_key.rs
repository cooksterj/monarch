//! The prefix indicator and the key popup that follows it.

use std::time::Instant;

use super::AttachApp;

impl AttachApp {
    /// The prefix was pressed (`true`) or its key arrived (`false`).
    pub(super) fn on_prefix(&mut self, pending: bool) {
        self.prefix_pending = pending;
        self.prefix_at = pending.then(Instant::now);
        self.popup = false;
        self.dirty = true;
    }

    /// Puts the popup up once the prefix has waited `which_key_delay`, so
    /// fast typists never see it flicker.
    pub(super) fn show_which_key(&mut self) {
        if !self.popup
            && self
                .prefix_at
                .is_some_and(|at| at.elapsed() >= self.opts.which_key_delay)
        {
            self.popup = true;
            self.dirty = true;
        }
    }
}
