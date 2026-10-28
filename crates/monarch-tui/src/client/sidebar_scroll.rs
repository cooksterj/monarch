//! Where the sidebar is scrolled to: moved by the wheel, kept within its
//! lines, and brought to the focused pane's row when focus moves.

use monarch_core::PaneId;

/// The sidebar's scroll offset and the focus it last followed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SidebarScroll {
    offset: u16,
    followed: Option<PaneId>,
}

impl SidebarScroll {
    /// Lines scrolled past at the top.
    #[must_use]
    pub fn offset(self) -> u16 {
        self.offset
    }

    /// Moves by `lines` (negative is up) through `total` lines shown
    /// `rows` at a time.
    pub fn scroll(&mut self, lines: i32, total: u16, rows: u16) {
        let moved = i32::from(self.offset).saturating_add(lines).max(0);
        self.offset = u16::try_from(moved).unwrap_or(u16::MAX);
        self.clamp(total, rows);
    }

    /// Keeps the last page full when the groups shrink.
    pub fn clamp(&mut self, total: u16, rows: u16) {
        self.offset = self.offset.min(total.saturating_sub(rows));
    }

    /// When focus has moved to `pane` since the last call, scrolls just
    /// enough to show its row, `line`. Scrolling by hand sticks until focus
    /// moves again.
    pub fn follow(&mut self, pane: Option<PaneId>, line: Option<u16>, rows: u16) {
        if pane == self.followed {
            return;
        }
        self.followed = pane;
        let Some(line) = line.filter(|_| rows > 0) else {
            return;
        };
        if line < self.offset {
            self.offset = line;
        } else if line >= self.offset + rows {
            self.offset = line + 1 - rows;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_moves_within_the_lines() {
        let mut s = SidebarScroll::default();
        s.scroll(-3, 40, 10);
        assert_eq!(s.offset(), 0, "not above the top");
        s.scroll(3, 40, 10);
        assert_eq!(s.offset(), 3);
        s.scroll(100, 40, 10);
        assert_eq!(s.offset(), 30, "the last page stays full");
        s.clamp(12, 10);
        assert_eq!(s.offset(), 2, "the groups shrank");
        s.scroll(3, 5, 10);
        assert_eq!(s.offset(), 0, "everything fits");
    }

    #[test]
    fn a_new_focus_is_brought_into_view_once() {
        let mut s = SidebarScroll::default();
        s.follow(Some(PaneId(7)), Some(25), 10);
        assert_eq!(s.offset(), 16, "its row is the last shown");
        s.scroll(-10, 40, 10);
        s.follow(Some(PaneId(7)), Some(25), 10);
        assert_eq!(s.offset(), 6, "scrolling away by hand sticks");
        s.follow(Some(PaneId(2)), Some(1), 10);
        assert_eq!(s.offset(), 1, "up to a row above");
        s.follow(Some(PaneId(3)), Some(5), 10);
        assert_eq!(s.offset(), 1, "already in view");
        s.follow(None, None, 10);
        assert_eq!(s.offset(), 1, "no focus, no move");
    }
}
