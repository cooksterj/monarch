//! The focused pane, shared with the input thread.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use monarch_core::PaneId;
use monarch_proto::ModeFlags;

use super::FocusView;

/// Lock-free hand-off from the main loop (which decodes frames) to the input
/// thread (which must know where keys go and how to filter them).
#[derive(Debug, Default)]
pub struct SharedFocus {
    pane: AtomicU32,
    modes: AtomicU32,
    rect: AtomicU64,
    scrolled: AtomicBool,
}

impl SharedFocus {
    /// Publishes the focused pane. The pane id is stored last, so a reader
    /// that sees it also sees the rest.
    pub fn set(&self, view: &FocusView) {
        let rect = [view.x, view.y, view.cols, view.rows]
            .iter()
            .fold(0u64, |acc, v| (acc << 16) | u64::from(*v));
        self.rect.store(rect, Ordering::Release);
        self.modes.store(view.modes.bits(), Ordering::Release);
        self.scrolled.store(view.scrolled, Ordering::Release);
        self.pane
            .store(view.pane.map_or(0, |p| p.0), Ordering::Release);
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

    /// Whether the focused pane shows scrollback.
    #[must_use]
    pub fn scrolled(&self) -> bool {
        self.scrolled.load(Ordering::Acquire)
    }

    /// Notes that a scroll-to-bottom was requested, so later keys do not
    /// request it again before the next frame arrives.
    pub fn clear_scrolled(&self) {
        self.scrolled.store(false, Ordering::Release);
    }

    /// Everything at once.
    #[must_use]
    pub fn view(&self) -> FocusView {
        let pane = self.pane();
        let rect = self.rect.load(Ordering::Acquire);
        let part = |shift: u32| (rect >> shift) as u16;
        FocusView {
            pane,
            x: part(48),
            y: part(32),
            cols: part(16),
            rows: part(0),
            modes: self.modes(),
            scrolled: self.scrolled(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_and_round_trips() {
        let focus = SharedFocus::default();
        assert_eq!(focus.view(), FocusView::default());
        let view = FocusView {
            pane: Some(PaneId(3)),
            x: 1,
            y: 2,
            cols: 300,
            rows: 65535,
            modes: ModeFlags::BRACKETED_PASTE,
            scrolled: true,
        };
        focus.set(&view);
        assert_eq!(focus.view(), view);
        assert_eq!(focus.pane(), Some(PaneId(3)));
        focus.clear_scrolled();
        assert!(!focus.scrolled());
    }

    #[test]
    fn wait_pane_sees_a_later_focus_and_gives_up_without_one() {
        let focus = std::sync::Arc::new(SharedFocus::default());
        assert_eq!(focus.wait_pane(Duration::from_millis(20)), None);
        let setter = std::sync::Arc::clone(&focus);
        let t = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            setter.set(&FocusView {
                pane: Some(PaneId(2)),
                ..FocusView::default()
            });
        });
        assert_eq!(focus.wait_pane(Duration::from_secs(5)), Some(PaneId(2)));
        t.join().expect("setter");
    }
}
