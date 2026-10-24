//! A run of adjacent cells that share a style and a width.

use serde::{Deserialize, Serialize};

use super::StyleId;

/// Consecutive cells of one row with the same style and width.
///
/// Normally each `char` of `text` is one cell (`cells == text.chars().count()`).
/// A cell with combining marks travels alone as a *cluster* run: `cells == 1`
/// and `text` is the base character followed by its marks. Width-2 cells
/// imply the zero-width spacer cell after them, which is not sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    /// Interned style of every cell in the run.
    pub style: StyleId,
    /// Column width of each cell: 1, 2 (wide) or 0 (orphan spacer).
    pub width: u8,
    /// Number of cells in the run.
    pub cells: u16,
    /// The cells' characters.
    pub text: String,
}

impl Run {
    /// Whether this is a cluster run (one cell carrying combining marks).
    #[must_use]
    pub fn is_cluster(&self) -> bool {
        self.cells == 1 && self.text.chars().nth(1).is_some()
    }

    /// Columns the run covers, spacers included.
    #[must_use]
    pub fn columns(&self) -> usize {
        usize::from(self.cells) * usize::from(self.width.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(width: u8, cells: u16, text: &str) -> Run {
        Run {
            style: StyleId::DEFAULT,
            width,
            cells,
            text: text.into(),
        }
    }

    #[test]
    fn clusters_are_single_cells_with_marks() {
        assert!(run(1, 1, "e\u{301}").is_cluster());
        assert!(!run(1, 1, "e").is_cluster());
        assert!(!run(1, 2, "ab").is_cluster());
    }

    #[test]
    fn columns_count_wide_cells_twice_and_spacers_once() {
        assert_eq!(run(2, 3, "中文字").columns(), 6);
        assert_eq!(run(1, 2, "ab").columns(), 2);
        assert_eq!(run(0, 1, " ").columns(), 1);
    }
}
