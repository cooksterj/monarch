//! A rectangle of terminal cells.

use serde::{Deserialize, Serialize};

/// Cells from (`x`, `y`), `cols` wide and `rows` tall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Rect {
    /// Left column.
    pub x: u16,
    /// Top row.
    pub y: u16,
    /// Width in cells.
    pub cols: u16,
    /// Height in cells.
    pub rows: u16,
}

impl Rect {
    /// A rectangle at (`x`, `y`) of `cols` × `rows`.
    #[must_use]
    pub const fn new(x: u16, y: u16, cols: u16, rows: u16) -> Self {
        Self { x, y, cols, rows }
    }

    /// One past the right edge.
    #[must_use]
    pub fn right(&self) -> u16 {
        self.x.saturating_add(self.cols)
    }

    /// One past the bottom edge.
    #[must_use]
    pub fn bottom(&self) -> u16 {
        self.y.saturating_add(self.rows)
    }

    /// Whether cell (`col`, `row`) is inside.
    #[must_use]
    pub fn contains(&self, col: u16, row: u16) -> bool {
        (self.x..self.right()).contains(&col) && (self.y..self.bottom()).contains(&row)
    }

    /// Number of cells.
    #[must_use]
    pub fn area(&self) -> u32 {
        u32::from(self.cols) * u32::from(self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_half_open() {
        let r = Rect::new(2, 3, 4, 5);
        assert_eq!((r.right(), r.bottom(), r.area()), (6, 8, 20));
        assert!(r.contains(2, 3) && r.contains(5, 7));
        assert!(!r.contains(6, 3) && !r.contains(2, 8));
    }

    #[test]
    fn empty_rects_contain_nothing() {
        assert!(!Rect::new(0, 0, 0, 5).contains(0, 0));
    }
}
