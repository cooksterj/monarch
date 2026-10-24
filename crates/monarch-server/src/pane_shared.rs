//! The part of a pane shared by its reader thread and the session.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use monarch_term::Terminal;

/// A pane's emulator plus its "has new output" flag. The reader thread
/// feeds the terminal and sets the flag; the frame coalescer clears the flag
/// and captures damaged rows. The lock is held only for those two steps.
pub struct PaneShared {
    term: Mutex<Terminal>,
    dirty: AtomicBool,
}

impl PaneShared {
    /// Wraps a terminal.
    #[must_use]
    pub fn new(term: Terminal) -> Self {
        Self {
            term: Mutex::new(term),
            dirty: AtomicBool::new(true),
        }
    }

    /// Locks the terminal. A panic while it was held leaves the emulator
    /// usable, so poisoning is ignored.
    pub fn lock(&self) -> MutexGuard<'_, Terminal> {
        self.term
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Marks new output; returns `true` if the pane was clean, i.e. the
    /// coalescer needs waking.
    pub fn mark_dirty(&self) -> bool {
        !self.dirty.swap(true, Ordering::AcqRel)
    }

    /// Clears the flag, returning whether it was set.
    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::ScreenSize;

    use super::*;

    #[test]
    fn dirty_flag_wakes_once_until_taken() {
        let shared = PaneShared::new(Terminal::new(ScreenSize::new(4, 2), 0));
        assert!(shared.take_dirty(), "new panes start dirty");
        assert!(shared.mark_dirty());
        assert!(!shared.mark_dirty(), "already dirty: no second wake-up");
        assert!(shared.take_dirty());
        assert!(!shared.take_dirty());
    }

    #[test]
    fn lock_gives_the_terminal() {
        let shared = PaneShared::new(Terminal::new(ScreenSize::new(4, 2), 0));
        shared.lock().feed(b"ab", &mut Vec::new());
        assert_eq!(shared.lock().text(), "ab\n");
    }
}
