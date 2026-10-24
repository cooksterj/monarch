//! Grid dimensions in cells.

use serde::{Deserialize, Serialize};

/// Width and height of a terminal grid, in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenSize {
    /// Columns.
    pub cols: u16,
    /// Rows.
    pub rows: u16,
}

impl ScreenSize {
    /// The smallest size any pane is allowed to have.
    pub const MIN: Self = Self { cols: 2, rows: 1 };

    /// A size, clamped to [`ScreenSize::MIN`].
    #[must_use]
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(Self::MIN.cols),
            rows: rows.max(Self::MIN.rows),
        }
    }

    /// Total number of cells.
    #[must_use]
    pub fn area(self) -> usize {
        usize::from(self.cols) * usize::from(self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_to_minimum() {
        assert_eq!(ScreenSize::new(0, 0), ScreenSize::MIN);
        assert_eq!(ScreenSize::new(80, 24).area(), 1920);
    }
}
