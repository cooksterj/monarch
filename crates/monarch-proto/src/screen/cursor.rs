//! Cursor position and appearance.

use serde::{Deserialize, Serialize};

use super::CursorShape;

/// Where the cursor is and how it should be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cursor {
    /// Row within the viewport.
    pub row: u16,
    /// Column.
    pub col: u16,
    /// Shape.
    pub shape: CursorShape,
    /// Whether it blinks.
    pub blinking: bool,
    /// Whether it is shown (DECTCEM, and not scrolled out of view).
    pub visible: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            shape: CursorShape::Block,
            blinking: false,
            visible: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_visible_at_origin() {
        let c = Cursor::default();
        assert!(c.visible);
        assert_eq!((c.row, c.col), (0, 0));
    }
}
