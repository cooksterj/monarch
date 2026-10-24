//! Where one pane sits in the client's window.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use crate::ScreenSize;

/// A pane's position, size and title within the layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneLayout {
    /// The pane.
    pub pane: PaneId,
    /// Left column.
    pub x: u16,
    /// Top row.
    pub y: u16,
    /// Size in cells.
    pub size: ScreenSize,
    /// Title set by the program, if any.
    pub title: Option<String>,
}

impl PaneLayout {
    /// Whether the window cell (`col`, `row`) falls inside this pane.
    #[must_use]
    pub fn contains(&self, col: u16, row: u16) -> bool {
        (self.x..self.x.saturating_add(self.size.cols)).contains(&col)
            && (self.y..self.y.saturating_add(self.size.rows)).contains(&row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_half_open() {
        let pane = PaneLayout {
            pane: PaneId(1),
            x: 10,
            y: 2,
            size: ScreenSize::new(5, 3),
            title: None,
        };
        assert!(pane.contains(10, 2));
        assert!(pane.contains(14, 4));
        assert!(!pane.contains(15, 2));
        assert!(!pane.contains(10, 5));
    }
}
