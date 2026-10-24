//! Moving a pane's divider (tmux `resize-pane`).

use super::geometry::{divide, first_length, length};
use super::{Node, SplitTree, WHOLE};
use crate::PaneId;
use crate::layout::{Direction, Rect};

impl SplitTree {
    /// Moves the nearest divider of `pane` that runs across `dir` by
    /// `cells` in that direction (so `Right` grows a left-hand pane and
    /// shrinks a right-hand one, as tmux does). Both sides keep at least
    /// one cell. Returns whether anything moved.
    pub fn resize(&mut self, area: Rect, pane: PaneId, dir: Direction, cells: u16) -> bool {
        let Some(root) = &mut self.root else {
            return false;
        };
        let Some((path, split_area)) = root.deepest_split(area, pane, dir) else {
            return false;
        };
        let Some(Node::Split { ratio, .. }) = root.at_path_mut(&path) else {
            return false;
        };
        let total = length(split_area, dir.axis());
        let avail = total.saturating_sub(1);
        if avail < 2 {
            return false;
        }
        let current = first_length(total, *ratio);
        let moved = if dir.is_forward() {
            current.saturating_add(cells)
        } else {
            current.saturating_sub(cells)
        }
        .clamp(1, avail - 1);
        if moved == current {
            return false;
        }
        // Round up so `first_length` gives back exactly `moved`.
        let scaled = (u32::from(moved) * u32::from(WHOLE)).div_ceil(u32::from(avail));
        *ratio = u16::try_from(scaled)
            .unwrap_or(WHOLE - 1)
            .clamp(1, WHOLE - 1);
        true
    }
}

impl Node {
    /// The path (`false` = first child) to the deepest split on `pane`'s
    /// way down whose axis matches `dir`, with that split's area.
    fn deepest_split(&self, area: Rect, pane: PaneId, dir: Direction) -> Option<(Vec<bool>, Rect)> {
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
            if *axis == dir.axis() {
                best = Some((path.clone(), area));
            }
            let (a, b) = divide(area, *axis, *ratio);
            let in_second = !first.panes_contain(pane);
            path.push(in_second);
            (node, area) = if in_second { (second, b) } else { (first, a) };
        }
        matches!(node, Self::Pane(p) if *p == pane).then_some(())?;
        best
    }

    fn panes_contain(&self, pane: PaneId) -> bool {
        let mut out = Vec::new();
        self.collect(&mut out);
        out.contains(&pane)
    }

    fn at_path_mut(&mut self, path: &[bool]) -> Option<&mut Self> {
        let mut node = self;
        for &second in path {
            let Self::Split {
                first, second: s, ..
            } = node
            else {
                return None;
            };
            node = if second { s } else { first };
        }
        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::SplitAxis;

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);
    const C: PaneId = PaneId(3);
    const AREA: Rect = Rect::new(0, 0, 81, 24);

    fn width(tree: &SplitTree, pane: PaneId) -> u16 {
        let rects = tree.layout(AREA);
        rects.iter().find(|(p, _)| *p == pane).expect("pane").1.cols
    }

    #[test]
    fn right_moves_the_divider_right_from_either_side() {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        assert!(tree.resize(AREA, A, Direction::Right, 5));
        assert_eq!((width(&tree, A), width(&tree, B)), (45, 35));
        assert!(
            tree.resize(AREA, B, Direction::Right, 5),
            "B's divider is the same one"
        );
        assert_eq!(width(&tree, A), 50);
    }

    #[test]
    fn panes_never_vanish() {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        tree.resize(AREA, A, Direction::Left, 500);
        assert_eq!(width(&tree, A), 1);
        assert!(
            !tree.resize(AREA, A, Direction::Left, 1),
            "already at the edge"
        );
    }

    #[test]
    fn the_nearest_matching_divider_moves() {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        tree.split(B, C, SplitAxis::Rows);
        assert!(
            tree.resize(AREA, C, Direction::Left, 4),
            "the column divider, above C"
        );
        assert_eq!(width(&tree, A), 36);
        assert!(
            !tree.resize(AREA, A, Direction::Up, 1),
            "A has no row divider"
        );
        assert!(!tree.resize(AREA, PaneId(9), Direction::Up, 1));
    }
}
