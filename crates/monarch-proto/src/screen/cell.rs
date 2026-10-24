//! One grid cell: a character (plus combining marks), its width and style.

use serde::{Deserialize, Serialize};

use super::CellStyle;

/// A grid cell. Wide characters occupy two cells: the first has `width: 2`,
/// the second is a spacer with `width: 0`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    /// The base character.
    pub ch: char,
    /// Zero-width characters (combining marks, variation selectors, ZWJ
    /// sequences) attached to `ch`. Empty for almost every cell, and an
    /// empty `Vec` does not allocate.
    pub combining: Vec<char>,
    /// Display width in cells: 1, 2 for wide, 0 for a wide-char spacer.
    pub width: u8,
    /// Colours and attributes.
    pub style: CellStyle,
}

impl Default for Cell {
    fn default() -> Self {
        Self::new(' ', 1)
    }
}

impl Cell {
    /// A plain cell holding `ch`.
    #[must_use]
    pub fn new(ch: char, width: u8) -> Self {
        Self {
            ch,
            combining: Vec::new(),
            width,
            style: CellStyle::default(),
        }
    }

    /// The spacer that follows a wide character.
    #[must_use]
    pub fn spacer() -> Self {
        Self::new(' ', 0)
    }

    /// The cell's full text (base character plus combining marks).
    #[must_use]
    pub fn text(&self) -> String {
        let mut s = String::with_capacity(4 + self.combining.len() * 4);
        s.push(self.ch);
        s.extend(&self.combining);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_includes_combining_marks() {
        let mut c = Cell::new('e', 1);
        c.combining.push('\u{301}');
        assert_eq!(c.text(), "e\u{301}");
    }

    #[test]
    fn default_is_a_blank() {
        let c = Cell::default();
        assert_eq!((c.ch, c.width), (' ', 1));
        assert!(c.style.is_plain());
    }
}
