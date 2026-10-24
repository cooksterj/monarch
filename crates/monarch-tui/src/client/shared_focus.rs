//! The focused pane and its modes, shared with the input thread.

use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use monarch_core::PaneId;
use monarch_proto::ModeFlags;

/// Lock-free hand-off from the main loop (which decodes frames) to the input
/// thread (which must know where keys go and how to filter them).
#[derive(Debug, Default)]
pub struct SharedFocus {
    pane: AtomicU32,
    modes: AtomicU32,
}

impl SharedFocus {
    /// Publishes the focused pane and its modes.
    pub fn set(&self, pane: Option<PaneId>, modes: ModeFlags) {
        self.modes.store(modes.bits(), Ordering::Release);
        self.pane.store(pane.map_or(0, |p| p.0), Ordering::Release);
    }

    /// The focused pane, if any.
    #[must_use]
    pub fn pane(&self) -> Option<PaneId> {
        match self.pane.load(Ordering::Acquire) {
            0 => None,
            id => Some(PaneId(id)),
        }
    }

    /// The focused pane, waiting up to `timeout` for the first layout so
    /// keys typed right after attaching are not lost. Polls only while no
    /// pane is known, which is only at startup.
    #[must_use]
    pub fn wait_pane(&self, timeout: Duration) -> Option<PaneId> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(pane) = self.pane() {
                return Some(pane);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// The focused pane's modes.
    #[must_use]
    pub fn modes(&self) -> ModeFlags {
        ModeFlags::from_bits_truncate(self.modes.load(Ordering::Acquire))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_and_round_trips() {
        let focus = SharedFocus::default();
        assert_eq!(focus.pane(), None);
        focus.set(Some(PaneId(3)), ModeFlags::BRACKETED_PASTE);
        assert_eq!(focus.pane(), Some(PaneId(3)));
        assert!(focus.modes().contains(ModeFlags::BRACKETED_PASTE));
    }

    #[test]
    fn wait_pane_sees_a_later_focus_and_gives_up_without_one() {
        let focus = std::sync::Arc::new(SharedFocus::default());
        assert_eq!(focus.wait_pane(Duration::from_millis(20)), None);
        let setter = std::sync::Arc::clone(&focus);
        let t = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            setter.set(Some(PaneId(2)), ModeFlags::empty());
        });
        assert_eq!(focus.wait_pane(Duration::from_secs(5)), Some(PaneId(2)));
        t.join().expect("setter");
    }
}
