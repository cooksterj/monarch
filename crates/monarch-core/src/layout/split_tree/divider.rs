//! Putting a pane's divider where a drag asks (a mouse resize).

use super::geometry::{divide, length};
use super::resize::set_first_length;
use super::{Node, SplitTree};
use crate::PaneId;
use crate::layout::{Direction, Rect};

impl SplitTree {
    /// Moves the divider on `pane`'s `dir` side so it sits on column (or
    /// row, for `Up`/`Down`) `to` of `area`, as near as the split allows:
    /// both sides keep at least one cell. Returns whether anything moved.
    /// Nothing happens when no divider bounds the pane on that side.
    pub fn set_divider(&mut self, area: Rect, pane: PaneId, dir: Direction, to: u16) -> bool {
        let Some(root) = &mut self.root else {
            return false;
        };
        let Some((path, split_area)) = root.bounding_split(area, pane, dir) else {
            return false;
        };
        let Some(Node::Split { ratio, .. }) = root.at_path_mut(&path) else {
            return false;
        };
        let start = match dir.axis() {
            crate::SplitAxis::Columns => split_area.x,
            crate::SplitAxis::Rows => split_area.y,
        };
        let total = length(split_area, dir.axis());
        set_first_length(ratio, total, to.saturating_sub(start))
    }
}

impl Node {
    /// The path (`false` = first child) to the deepest split on `pane`'s
    /// way down whose divider is `pane`'s edge in `dir`: its axis matches
    /// and the pane is on the near side (first child for `Right`/`Down`,
    /// second for `Left`/`Up`). With that split's area.
    fn bounding_split(
        &self,
        area: Rect,
        pane: PaneId,
        dir: Direction,
    ) -> Option<(Vec<bool>, Rect)> {
        let mut node = self;
        let mut area = area;
        let mut path = Vec::new();
        let mut best = None;
        while let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = node
        {
            let (a, b) = divide(area, *axis, *ratio);
            let in_second = !first.panes_contain(pane);
            if *axis == dir.axis() && in_second != dir.is_forward() {
                best = Some((path.clone(), area));
            }
            path.push(in_second);
            (node, area) = if in_second { (second, b) } else { (first, a) };
        }
        matches!(node, Self::Pane(p) if *p == pane).then_some(())?;
        best
    }
}

/// Used by tests only: where the divider of the split that owns `ratio`
/// sits, to check `set_divider` landed.
#[cfg(test)]
fn divider_of(area: Rect, ratio: u16, axis: crate::SplitAxis) -> u16 {
    let start = match axis {
        crate::SplitAxis::Columns => area.x,
        crate::SplitAxis::Rows => area.y,
    };
    start + super::geometry::first_length(length(area, axis), ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitAxis;

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);
    const C: PaneId = PaneId(3);
    const AREA: Rect = Rect::new(0, 0, 100, 40);

    fn rect(tree: &SplitTree, pane: PaneId) -> Rect {
        tree.layout(AREA)
            .into_iter()
            .find(|(p, _)| *p == pane)
            .expect("pane")
            .1
    }

    /// `[A | B]` over `C`: a column split inside the first row of a row
    /// split.
    fn nested() -> SplitTree {
        let mut tree = SplitTree::new(A);
        tree.split(A, C, SplitAxis::Rows);
        tree.split(A, B, SplitAxis::Columns);
        tree
    }

    #[test]
    fn the_divider_lands_on_the_column_asked_from_either_side() {
        let mut tree = nested();
        assert!(tree.set_divider(AREA, A, Direction::Right, 30));
        assert_eq!(rect(&tree, A).right(), 30, "A ends at the divider");
        assert_eq!(rect(&tree, B).x, 31, "B starts after it");
        assert!(
            tree.set_divider(AREA, B, Direction::Left, 70),
            "B's left edge is the same divider"
        );
        assert_eq!(rect(&tree, A).right(), 70);
        assert!(
            !tree.set_divider(AREA, B, Direction::Left, 70),
            "already there"
        );
    }

    #[test]
    fn rows_move_the_same_way_and_the_ratio_is_exact() {
        let mut tree = nested();
        assert!(tree.set_divider(AREA, C, Direction::Up, 25));
        assert_eq!(rect(&tree, C).y, 26);
        assert_eq!(rect(&tree, A).bottom(), 25);
        assert_eq!(
            rect(&tree, B).bottom(),
            25,
            "both top panes end at the divider"
        );
        let ratio = match tree.root.as_ref().expect("root") {
            Node::Split { ratio, .. } => *ratio,
            Node::Pane(_) => unreachable!(),
        };
        assert_eq!(divider_of(AREA, ratio, SplitAxis::Rows), 25);
    }

    #[test]
    fn both_sides_keep_a_cell_whatever_is_asked() {
        let mut tree = nested();
        assert!(tree.set_divider(AREA, A, Direction::Right, 0));
        assert_eq!(rect(&tree, A).cols, 1);
        assert!(tree.set_divider(AREA, A, Direction::Right, 500));
        assert_eq!(rect(&tree, B).cols, 1);
        assert_eq!(rect(&tree, A).cols, 98);
    }

    #[test]
    fn a_side_with_no_divider_does_nothing() {
        let mut tree = nested();
        let before = tree.layout(AREA);
        assert!(
            !tree.set_divider(AREA, A, Direction::Left, 10),
            "A touches the area's edge"
        );
        assert!(!tree.set_divider(AREA, C, Direction::Down, 10));
        assert!(
            !tree.set_divider(AREA, C, Direction::Right, 10),
            "C spans the width"
        );
        assert!(
            !tree.set_divider(AREA, PaneId(9), Direction::Right, 10),
            "no such pane"
        );
        assert_eq!(tree.layout(AREA), before);
        assert!(!SplitTree::new(A).set_divider(AREA, A, Direction::Right, 10));
    }

    #[test]
    fn the_bounding_split_is_the_nearest_one_on_that_side() {
        // A | [B | C]: B's right edge is the inner divider, its left the outer.
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        tree.split(B, C, SplitAxis::Columns);
        assert!(tree.set_divider(AREA, B, Direction::Right, 80));
        assert_eq!(rect(&tree, C).x, 81);
        assert!(tree.set_divider(AREA, B, Direction::Left, 20));
        assert_eq!(rect(&tree, A).right(), 20);
        assert_eq!(rect(&tree, B).x, 21);
        // The inner split keeps its ratio of a wider area, so its divider
        // shifts with it rather than staying on column 80.
        assert!(rect(&tree, C).x > rect(&tree, B).right());
    }
}
