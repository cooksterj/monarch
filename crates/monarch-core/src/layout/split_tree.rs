//! A tab's panes as a binary tree of splits (tmux's layout model).
//!
//! Each split divides its area between two children along an axis, at a
//! ratio in thousandths, leaving one cell for the divider line. The tree is
//! pure data: geometry (`layout`, `neighbour`, `pane_at`) and `resize` live
//! in the child modules and take the area to fill as an argument.

mod geometry;
mod resize;

use serde::{Deserialize, Serialize};

use super::SplitAxis;
use crate::PaneId;

/// An even split, in thousandths.
const HALF: u16 = 500;
/// The ratio scale.
const WHOLE: u16 = 1000;

/// One node: a pane, or a split of two subtrees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Node {
    Pane(PaneId),
    Split {
        axis: SplitAxis,
        /// The first child's share, in thousandths.
        ratio: u16,
        first: Box<Node>,
        second: Box<Node>,
    },
}

/// The arrangement of one tab's panes. Empty once its last pane closes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitTree {
    root: Option<Node>,
}

impl SplitTree {
    /// A tree holding just `pane`.
    #[must_use]
    pub fn new(pane: PaneId) -> Self {
        Self {
            root: Some(Node::Pane(pane)),
        }
    }

    /// Whether no panes remain.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Panes in reading order (left to right, top to bottom).
    #[must_use]
    pub fn panes(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            root.collect(&mut out);
        }
        out
    }

    /// Whether `pane` is in the tree.
    #[must_use]
    pub fn contains(&self, pane: PaneId) -> bool {
        self.panes().contains(&pane)
    }

    /// Splits `target` in half along `axis`; `new` takes the second half
    /// (right or below). Returns `false` if `target` is not in the tree.
    pub fn split(&mut self, target: PaneId, new: PaneId, axis: SplitAxis) -> bool {
        let Some(leaf) = self.root.as_mut().and_then(|r| r.find_mut(target)) else {
            return false;
        };
        *leaf = Node::Split {
            axis,
            ratio: HALF,
            first: Box::new(Node::Pane(target)),
            second: Box::new(Node::Pane(new)),
        };
        true
    }

    /// Removes `pane`; its sibling takes the space. Returns `false` if it
    /// was not in the tree.
    pub fn remove(&mut self, pane: PaneId) -> bool {
        let Some(root) = self.root.take() else {
            return false;
        };
        let (rest, found) = root.without(pane);
        self.root = rest;
        found
    }

    /// Swaps two panes' places. Returns `false` unless both are present.
    pub fn swap(&mut self, a: PaneId, b: PaneId) -> bool {
        if a == b || !self.contains(a) || !self.contains(b) {
            return false;
        }
        if let Some(root) = &mut self.root {
            root.map_panes(&mut |p| {
                if p == a {
                    b
                } else if p == b {
                    a
                } else {
                    p
                }
            });
        }
        true
    }

    /// Gives every pane the same share of its axis, whatever the nesting.
    pub fn equalize(&mut self) {
        if let Some(root) = &mut self.root {
            root.equalize();
        }
    }
}

impl Node {
    fn collect(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Pane(p) => out.push(*p),
            Self::Split { first, second, .. } => {
                first.collect(out);
                second.collect(out);
            }
        }
    }

    fn find_mut(&mut self, pane: PaneId) -> Option<&mut Self> {
        match self {
            Self::Pane(p) if *p == pane => Some(self),
            Self::Pane(_) => None,
            Self::Split { first, second, .. } => {
                first.find_mut(pane).or_else(|| second.find_mut(pane))
            }
        }
    }

    /// This subtree without `pane`, and whether it was found.
    fn without(self, pane: PaneId) -> (Option<Self>, bool) {
        match self {
            Self::Pane(p) if p == pane => (None, true),
            Self::Pane(_) => (Some(self), false),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => match first.without(pane) {
                (None, _) => (Some(*second), true),
                (Some(first), true) => (Some(Self::split(axis, ratio, first, *second)), true),
                (Some(first), false) => match second.without(pane) {
                    (None, _) => (Some(first), true),
                    (Some(second), found) => (Some(Self::split(axis, ratio, first, second)), found),
                },
            },
        }
    }

    fn split(axis: SplitAxis, ratio: u16, first: Self, second: Self) -> Self {
        Self::Split {
            axis,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    fn map_panes(&mut self, f: &mut impl FnMut(PaneId) -> PaneId) {
        match self {
            Self::Pane(p) => *p = f(*p),
            Self::Split { first, second, .. } => {
                first.map_panes(f);
                second.map_panes(f);
            }
        }
    }

    /// Panes along `axis` in this subtree (a perpendicular split counts as
    /// one, since its panes share the same span).
    fn span_count(&self, axis: SplitAxis) -> u16 {
        match self {
            Self::Pane(_) => 1,
            Self::Split {
                axis: a,
                first,
                second,
                ..
            } if *a == axis => first.span_count(axis) + second.span_count(axis),
            Self::Split { .. } => 1,
        }
    }

    fn equalize(&mut self) {
        if let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        {
            let (a, b) = (first.span_count(*axis), second.span_count(*axis));
            *ratio = WHOLE * a / (a + b);
            first.equalize();
            second.equalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);
    const C: PaneId = PaneId(3);

    #[test]
    fn splits_keep_reading_order() {
        let mut tree = SplitTree::new(A);
        assert!(tree.split(A, B, SplitAxis::Columns));
        assert!(tree.split(A, C, SplitAxis::Rows));
        assert_eq!(tree.panes(), [A, C, B]);
        assert!(!tree.split(PaneId(9), C, SplitAxis::Rows), "unknown target");
    }

    #[test]
    fn removing_a_pane_gives_its_space_to_the_sibling() {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        tree.split(B, C, SplitAxis::Rows);
        assert!(tree.remove(B));
        assert_eq!(tree.panes(), [A, C]);
        assert!(!tree.remove(B), "already gone");
        assert!(tree.remove(A) && tree.remove(C));
        assert!(tree.is_empty());
        assert!(!tree.remove(A));
    }

    #[test]
    fn swap_exchanges_places() {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        assert!(tree.swap(A, B));
        assert_eq!(tree.panes(), [B, A]);
        assert!(!tree.swap(A, PaneId(9)));
    }

    #[test]
    fn equalize_shares_a_row_of_three_evenly() {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Columns);
        tree.split(B, C, SplitAxis::Columns);
        tree.equalize();
        let Some(Node::Split { ratio, .. }) = &tree.root else {
            panic!("split root");
        };
        assert_eq!(*ratio, 333, "A gets a third, B and C share the rest");
    }

    #[test]
    fn round_trips_through_json() {
        let mut tree = SplitTree::new(A);
        tree.split(A, B, SplitAxis::Rows);
        let json = serde_json::to_string(&tree).expect("json");
        assert_eq!(
            serde_json::from_str::<SplitTree>(&json).expect("parse"),
            tree
        );
    }
}
