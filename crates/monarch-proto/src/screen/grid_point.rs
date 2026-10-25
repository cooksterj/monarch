//! A cell addressed by absolute line, so it stays put as output scrolls.

use serde::{Deserialize, Serialize};

use super::Screen;

/// A cell in a pane's whole grid: line 0 is the oldest scrollback line, so
/// the same text keeps the same line while new output pushes it upwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GridPoint {
    /// Absolute line, scrollback included.
    pub line: i64,
    /// Column.
    pub col: u16,
}

impl GridPoint {
    /// The absolute line shown at the top of `screen`'s viewport.
    #[must_use]
    pub fn top_line(screen: &Screen) -> i64 {
        i64::from(screen.history) - i64::from(screen.display_offset)
    }

    /// The grid point under viewport cell (`row`, `col`).
    #[must_use]
    pub fn at(screen: &Screen, row: u16, col: u16) -> Self {
        Self {
            line: Self::top_line(screen) + i64::from(row),
            col,
        }
    }

    /// This point's viewport row (negative above the viewport).
    #[must_use]
    pub fn viewport_row(&self, screen: &Screen) -> i64 {
        self.line - Self::top_line(screen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScreenSize;

    #[test]
    fn points_keep_their_line_as_history_grows() {
        let mut screen = Screen::blank(ScreenSize::new(10, 5));
        screen.history = 100;
        let p = GridPoint::at(&screen, 2, 3);
        assert_eq!(p, GridPoint { line: 102, col: 3 });
        screen.history = 101; // one line of output scrolled everything up
        assert_eq!(p.viewport_row(&screen), 1);
        screen.display_offset = 4; // the user scrolled back
        assert_eq!(p.viewport_row(&screen), 5);
    }
}
