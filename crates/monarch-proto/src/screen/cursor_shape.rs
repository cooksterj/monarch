//! The cursor shapes a program can request with DECSCUSR.

use serde::{Deserialize, Serialize};

/// Cursor shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum CursorShape {
    /// Filled block.
    #[default]
    Block,
    /// Underline.
    Underline,
    /// Vertical bar.
    Beam,
}

impl CursorShape {
    /// The DECSCUSR parameter (`CSI <n> SP q`) for this shape.
    #[must_use]
    pub fn decscusr(self, blinking: bool) -> u8 {
        let steady = match self {
            Self::Block => 2,
            Self::Underline => 4,
            Self::Beam => 6,
        };
        if blinking { steady - 1 } else { steady }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decscusr_codes_match_the_vt_spec() {
        assert_eq!(CursorShape::Block.decscusr(true), 1);
        assert_eq!(CursorShape::Block.decscusr(false), 2);
        assert_eq!(CursorShape::Underline.decscusr(false), 4);
        assert_eq!(CursorShape::Beam.decscusr(true), 5);
    }
}
