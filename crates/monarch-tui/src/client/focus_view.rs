//! What the input thread needs to know about the focused pane.

use monarch_core::PaneId;
use monarch_proto::ModeFlags;

/// The focused pane, where it sits on the host and how it wants input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FocusView {
    /// The pane, if any.
    pub pane: Option<PaneId>,
    /// Left column on the host.
    pub x: u16,
    /// Top row on the host.
    pub y: u16,
    /// Width in cells.
    pub cols: u16,
    /// Height in cells.
    pub rows: u16,
    /// Its terminal modes.
    pub modes: ModeFlags,
    /// It shows scrollback rather than live output.
    pub scrolled: bool,
}

impl FocusView {
    /// The pane-local cell for host cell (`col`, `row`), if inside.
    #[must_use]
    pub fn local(&self, col: u16, row: u16) -> Option<(u16, u16)> {
        let inside = (self.x..self.x.saturating_add(self.cols)).contains(&col)
            && (self.y..self.y.saturating_add(self.rows)).contains(&row);
        inside.then(|| (col - self.x, row - self.y))
    }

    /// Like [`Self::local`] but clamped into the pane, for drags that
    /// leave it.
    #[must_use]
    pub fn clamped(&self, col: u16, row: u16) -> (u16, u16) {
        let max = |n: u16| n.saturating_sub(1);
        (
            col.saturating_sub(self.x).min(max(self.cols)),
            row.saturating_sub(self.y).min(max(self.rows)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> FocusView {
        FocusView {
            pane: Some(PaneId(1)),
            x: 10,
            y: 2,
            cols: 5,
            rows: 3,
            ..FocusView::default()
        }
    }

    #[test]
    fn local_maps_inside_cells_only() {
        assert_eq!(view().local(10, 2), Some((0, 0)));
        assert_eq!(view().local(14, 4), Some((4, 2)));
        assert_eq!(view().local(15, 2), None);
        assert_eq!(view().local(9, 2), None);
    }

    #[test]
    fn clamped_keeps_drags_on_the_pane_edge() {
        assert_eq!(view().clamped(0, 0), (0, 0));
        assert_eq!(view().clamped(99, 99), (4, 2));
    }
}
