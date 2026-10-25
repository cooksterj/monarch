//! The welcome screen: opened on the first attach (or from the palette),
//! animated while open, closed by any key.

use std::time::{Instant, SystemTime};

use super::AttachApp;
use crate::{WELCOME_STEP, WelcomeState};

impl AttachApp {
    /// Shows the welcome.
    pub(super) fn open_welcome(&mut self) {
        let now = Instant::now();
        // A different swarm each time; the clock is random enough.
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| {
                u64::try_from(d.as_nanos() & u128::from(u64::MAX)).unwrap_or(0)
            });
        let welcome = WelcomeState::new(&self.opts.keymap, &self.opts.prefix_label, now, seed);
        self.popups.welcome = Some(welcome);
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Any key closes the welcome; none reaches the pane.
    pub(super) fn welcome_input(&mut self) {
        self.popups.welcome = None;
        self.focus.set_overlay(self.any_overlay());
        self.dirty = true;
    }

    /// Asks for the welcome's next frame when it is due. True while the
    /// welcome is open, so other animations wait.
    pub(super) fn animate_welcome(&mut self) -> bool {
        if self.popups.welcome.is_none() {
            return false;
        }
        let now = Instant::now();
        if self.opts.animations && now >= self.glyphs.next_frame {
            self.dirty = true;
            self.glyphs.next_frame = now + WELCOME_STEP;
        }
        true
    }
}
