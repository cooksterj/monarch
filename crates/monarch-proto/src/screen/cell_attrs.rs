//! Text attributes of a cell (bold, italic, underline styles, …).

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// SGR text attributes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct CellAttrs: u16 {
        /// Bold / increased intensity.
        const BOLD = 1 << 0;
        /// Faint / decreased intensity.
        const DIM = 1 << 1;
        /// Italic.
        const ITALIC = 1 << 2;
        /// Single underline.
        const UNDERLINE = 1 << 3;
        /// Double underline.
        const DOUBLE_UNDERLINE = 1 << 4;
        /// Curly underline.
        const UNDERCURL = 1 << 5;
        /// Dotted underline.
        const DOTTED_UNDERLINE = 1 << 6;
        /// Dashed underline.
        const DASHED_UNDERLINE = 1 << 7;
        /// Swap foreground and background.
        const INVERSE = 1 << 8;
        /// Invisible text.
        const HIDDEN = 1 << 9;
        /// Strikethrough.
        const STRIKEOUT = 1 << 10;
        /// Any underline style.
        const ANY_UNDERLINE = Self::UNDERLINE.bits() | Self::DOUBLE_UNDERLINE.bits()
            | Self::UNDERCURL.bits() | Self::DOTTED_UNDERLINE.bits()
            | Self::DASHED_UNDERLINE.bits();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_underline_covers_every_style() {
        for u in [
            CellAttrs::UNDERLINE,
            CellAttrs::UNDERCURL,
            CellAttrs::DASHED_UNDERLINE,
        ] {
            assert!(CellAttrs::ANY_UNDERLINE.contains(u));
        }
        assert!(!CellAttrs::ANY_UNDERLINE.contains(CellAttrs::BOLD));
    }
}
