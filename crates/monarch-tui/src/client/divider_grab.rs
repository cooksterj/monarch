//! What a mouse press on a divider takes hold of: a pane's edge, to be
//! dragged into a resize, or the sidebar's edge.

use monarch_core::{Direction, PaneId, Rect, SplitAxis};

/// The divider under a press. Pure data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerGrab {
    /// The divider on `pane`'s `dir` side (`Right` for a vertical one,
    /// `Down` for a horizontal one: the pane before it in reading order).
    Pane {
        /// The pane the divider borders.
        pane: PaneId,
        /// Which of its sides.
        dir: Direction,
    },
    /// The sidebar's right edge.
    Sidebar,
}

impl DividerGrab {
    /// What a press at host cell (`col`, `row`) grabs, given every visible
    /// pane's host rectangle and the `left` columns the sidebar takes
    /// (0 when hidden). The sidebar's last column is its edge. A pane
    /// divider is a cell no pane covers with panes on both sides of it (a
    /// crossing counts for both dividers meeting there, and the vertical
    /// one wins). Anything else is `None`.
    #[must_use]
    pub fn at(rects: &[(PaneId, Rect)], left: u16, col: u16, row: u16) -> Option<Self> {
        if left > 0 && col + 1 == left {
            return Some(Self::Sidebar);
        }
        let panes: Vec<&(PaneId, Rect)> = rects.iter().filter(|(_, r)| r.x >= left).collect();
        if col < left || panes.iter().any(|(_, r)| r.contains(col, row)) {
            return None;
        }
        let before = |axis| {
            panes.iter().find(|(_, r)| match axis {
                SplitAxis::Columns => r.right() == col && (r.y..=r.bottom()).contains(&row),
                SplitAxis::Rows => r.bottom() == row && (r.x..=r.right()).contains(&col),
            })
        };
        let after = |axis| {
            panes.iter().any(|(_, r)| match axis {
                SplitAxis::Columns => r.x == col + 1 && (r.y..=r.bottom()).contains(&row),
                SplitAxis::Rows => r.y == row + 1 && (r.x..=r.right()).contains(&col),
            })
        };
        for (axis, dir) in [
            (SplitAxis::Columns, Direction::Right),
            (SplitAxis::Rows, Direction::Down),
        ] {
            if let Some((pane, _)) = before(axis)
                && after(axis)
            {
                return Some(Self::Pane { pane: *pane, dir });
            }
        }
        None
    }

    /// Where a drag to host cell (`col`, `row`) puts a pane divider, in
    /// pane-area cells (host columns less the sidebar's `left`).
    #[must_use]
    pub fn target(self, left: u16, col: u16, row: u16) -> u16 {
        match self {
            Self::Pane { dir, .. } if dir.axis() == SplitAxis::Rows => row,
            Self::Pane { .. } => col.saturating_sub(left),
            Self::Sidebar => col.saturating_add(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);
    const C: PaneId = PaneId(3);

    /// Sidebar 0..10, then `[A | B]` over `C` in a 40×20 pane area.
    fn rects() -> Vec<(PaneId, Rect)> {
        vec![
            (A, Rect::new(10, 0, 15, 9)),
            (B, Rect::new(26, 0, 24, 9)),
            (C, Rect::new(10, 10, 40, 10)),
            // Sidebar rows carry pane ids too; they must not count.
            (A, Rect::new(0, 1, 9, 1)),
            (C, Rect::new(0, 2, 9, 1)),
        ]
    }

    #[test]
    fn a_vertical_divider_is_the_pane_before_it_and_right() {
        let grab = DividerGrab::at(&rects(), 10, 25, 4);
        assert_eq!(
            grab,
            Some(DividerGrab::Pane {
                pane: A,
                dir: Direction::Right
            })
        );
        assert_eq!(
            grab.expect("grab").target(10, 33, 7),
            23,
            "host column less the sidebar"
        );
    }

    #[test]
    fn a_horizontal_divider_is_the_pane_above_it_and_down() {
        let grab = DividerGrab::at(&rects(), 10, 30, 9);
        assert_eq!(
            grab,
            Some(DividerGrab::Pane {
                pane: B,
                dir: Direction::Down
            })
        );
        assert_eq!(
            grab.expect("grab").target(10, 30, 14),
            14,
            "rows are not shifted"
        );
    }

    #[test]
    fn the_crossing_takes_the_vertical_one() {
        assert_eq!(
            DividerGrab::at(&rects(), 10, 25, 9),
            Some(DividerGrab::Pane {
                pane: A,
                dir: Direction::Right
            })
        );
    }

    #[test]
    fn the_sidebar_edge_is_its_last_column() {
        assert_eq!(
            DividerGrab::at(&rects(), 10, 9, 5),
            Some(DividerGrab::Sidebar)
        );
        assert_eq!(DividerGrab::Sidebar.target(10, 15, 5), 16, "the new width");
        assert_eq!(
            DividerGrab::at(&rects(), 0, 9, 5),
            None,
            "no sidebar: column 9 is nothing"
        );
        assert_eq!(
            DividerGrab::at(&rects(), 10, 4, 5),
            None,
            "inside the sidebar"
        );
    }

    #[test]
    fn panes_edges_and_the_status_row_grab_nothing() {
        let rects = rects();
        assert_eq!(DividerGrab::at(&rects, 10, 12, 3), None, "inside A");
        assert_eq!(
            DividerGrab::at(&rects, 10, 30, 20),
            None,
            "below C: the status line"
        );
        assert_eq!(
            DividerGrab::at(&rects, 10, 50, 4),
            None,
            "past B: the host's edge"
        );
        assert_eq!(DividerGrab::at(&[], 0, 5, 5), None);
    }
}
