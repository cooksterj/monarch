//! Turning a split tree into pane rectangles, and questions about them.

use super::{Node, SplitTree, WHOLE};
use crate::PaneId;
use crate::layout::{Direction, Rect, SplitAxis};

impl SplitTree {
    /// Each pane's rectangle when the tree fills `area`, in reading order.
    /// Splits leave a one-cell divider between their children.
    #[must_use]
    pub fn layout(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            root.place(area, &mut out);
        }
        out
    }

    /// The pane whose rectangle holds cell (`col`, `row`).
    #[must_use]
    pub fn pane_at(&self, area: Rect, col: u16, row: u16) -> Option<PaneId> {
        self.layout(area)
            .into_iter()
            .find(|(_, r)| r.contains(col, row))
            .map(|(p, _)| p)
    }

    /// The pane across the divider from `pane` in `dir`: of those whose
    /// facing edge touches it, the one sharing the most of its edge (then
    /// the topmost or leftmost).
    #[must_use]
    pub fn neighbour(&self, area: Rect, pane: PaneId, dir: Direction) -> Option<PaneId> {
        let rects = self.layout(area);
        let from = rects.iter().find(|(p, _)| *p == pane)?.1;
        rects
            .iter()
            .filter(|(p, _)| *p != pane)
            .filter_map(|(p, r)| facing_overlap(from, *r, dir).map(|o| (*p, o, *r)))
            .max_by_key(|(_, overlap, r)| (*overlap, std::cmp::Reverse((r.y, r.x))))
            .map(|(p, _, _)| p)
    }
}

impl Node {
    fn place(&self, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match self {
            Self::Pane(p) => out.push((*p, area)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let (a, b) = divide(area, *axis, *ratio);
                first.place(a, out);
                second.place(b, out);
            }
        }
    }
}

/// Cells along `axis` in `area`.
pub(super) fn length(area: Rect, axis: SplitAxis) -> u16 {
    match axis {
        SplitAxis::Columns => area.cols,
        SplitAxis::Rows => area.rows,
    }
}

/// The first child's length along the axis: its share of what the
/// divider leaves, keeping both children at least one cell when possible.
pub(super) fn first_length(total: u16, ratio: u16) -> u16 {
    let avail = total.saturating_sub(1);
    if avail < 2 {
        return avail;
    }
    let share = u32::from(avail) * u32::from(ratio) / u32::from(WHOLE);
    u16::try_from(share).unwrap_or(avail).clamp(1, avail - 1)
}

/// Splits `area` into the two children's rectangles around a divider.
pub(super) fn divide(area: Rect, axis: SplitAxis, ratio: u16) -> (Rect, Rect) {
    let total = length(area, axis);
    let a = first_length(total, ratio);
    let b = total.saturating_sub(a + 1);
    match axis {
        SplitAxis::Columns => (
            Rect::new(area.x, area.y, a, area.rows),
            Rect::new(area.x + a + 1, area.y, b, area.rows),
        ),
        SplitAxis::Rows => (
            Rect::new(area.x, area.y, area.cols, a),
            Rect::new(area.x, area.y + a + 1, area.cols, b),
        ),
    }
}

/// How much of `from`'s edge `to` shares across one divider in `dir`.
fn facing_overlap(from: Rect, to: Rect, dir: Direction) -> Option<u16> {
    let touches = match dir {
        Direction::Left => to.right() + 1 == from.x,
        Direction::Right => from.right() + 1 == to.x,
        Direction::Up => to.bottom() + 1 == from.y,
        Direction::Down => from.bottom() + 1 == to.y,
    };
    let (lo, hi) = match dir.axis() {
        SplitAxis::Columns => (from.y.max(to.y), from.bottom().min(to.bottom())),
        SplitAxis::Rows => (from.x.max(to.x), from.right().min(to.right())),
    };
    (touches && hi > lo).then(|| hi - lo)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);
    const C: PaneId = PaneId(3);
    const AREA: Rect = Rect::new(0, 0, 81, 24);

    /// A | (B over C)
    fn tree() -> SplitTree {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        tree.split(B, C, SplitAxis::Rows);
        tree
    }

    #[test]
    fn splits_leave_a_divider() {
        let rects = tree().layout(AREA);
        assert_eq!(
            rects,
            [
                (A, Rect::new(0, 0, 40, 24)),
                (B, Rect::new(41, 0, 40, 11)),
                (C, Rect::new(41, 12, 40, 12)),
            ]
        );
    }

    #[test]
    fn neighbours_are_found_across_dividers() {
        let tree = tree();
        assert_eq!(
            tree.neighbour(AREA, A, Direction::Right),
            Some(C),
            "C (12 rows) shares more of A's edge than B (11)"
        );
        let even = Rect::new(0, 0, 81, 23);
        assert_eq!(
            tree.neighbour(even, A, Direction::Right),
            Some(B),
            "topmost wins a tie"
        );
        assert_eq!(tree.neighbour(AREA, C, Direction::Left), Some(A));
        assert_eq!(tree.neighbour(AREA, B, Direction::Down), Some(C));
        assert_eq!(
            tree.neighbour(AREA, A, Direction::Left),
            None,
            "window edge"
        );
        assert_eq!(tree.neighbour(AREA, PaneId(9), Direction::Up), None);
    }

    #[test]
    fn cells_map_to_panes_and_dividers_to_none() {
        let tree = tree();
        assert_eq!(tree.pane_at(AREA, 0, 0), Some(A));
        assert_eq!(tree.pane_at(AREA, 40, 5), None, "divider");
        assert_eq!(tree.pane_at(AREA, 50, 20), Some(C));
    }

    #[test]
    fn tiny_areas_never_underflow() {
        let tree = tree();
        for (cols, rows) in [(0, 0), (1, 1), (2, 2), (3, 1)] {
            let _ = tree.layout(Rect::new(0, 0, cols, rows));
        }
    }

    proptest! {
        #[test]
        fn panes_tile_the_area_without_overlap(
            ops in proptest::collection::vec((0usize..8, any::<bool>(), any::<bool>()), 1..12),
            cols in 40u16..240,
            rows in 20u16..80,
        ) {
            let mut tree = SplitTree::new(PaneId(1));
            let mut next = 2;
            for (pick, columns, remove) in ops {
                let panes = tree.panes();
                let target = panes[pick % panes.len()];
                if remove && panes.len() > 1 {
                    tree.remove(target);
                } else {
                    let axis = if columns { SplitAxis::Columns } else { SplitAxis::Rows };
                    tree.split(target, PaneId(next), axis);
                    next += 1;
                }
            }
            let area = Rect::new(0, 0, cols, rows);
            let rects = tree.layout(area);
            prop_assert_eq!(rects.len(), tree.panes().len());
            for (i, (_, a)) in rects.iter().enumerate() {
                prop_assert!(a.right() <= area.right() && a.bottom() <= area.bottom());
                for (_, b) in &rects[i + 1..] {
                    let disjoint = a.right() <= b.x || b.right() <= a.x
                        || a.bottom() <= b.y || b.bottom() <= a.y;
                    prop_assert!(disjoint || a.area() == 0 || b.area() == 0, "{:?} {:?}", a, b);
                }
            }
        }
    }
}
