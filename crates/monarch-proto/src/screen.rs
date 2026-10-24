//! A pane's visible screen: a grid of styled cells plus cursor and modes.

mod cell;
mod cell_attrs;
mod cell_style;
mod cursor;
mod cursor_shape;
mod mode_flags;
mod rgb;
mod screen_size;
mod term_color;
mod terminal_modes;

use serde::{Deserialize, Serialize};

pub use cell::Cell;
pub use cell_attrs::CellAttrs;
pub use cell_style::CellStyle;
pub use cursor::Cursor;
pub use cursor_shape::CursorShape;
pub use mode_flags::ModeFlags;
pub use rgb::Rgb;
pub use screen_size::ScreenSize;
pub use term_color::TermColor;
pub use terminal_modes::TerminalModes;

/// The visible viewport of one pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    /// Grid dimensions; `rows.len() == size.rows` and every row has
    /// `size.cols` cells.
    pub size: ScreenSize,
    /// Rows top to bottom.
    pub rows: Vec<Vec<Cell>>,
    /// Cursor position and appearance.
    pub cursor: Cursor,
    /// Terminal modes the program inside the pane has enabled.
    pub modes: TerminalModes,
    /// How many lines the viewport is scrolled back into history (0 = live).
    pub display_offset: u32,
    /// Lines of scrollback available above the live screen.
    pub history: u32,
}

impl Screen {
    /// A blank screen of the given size.
    #[must_use]
    pub fn blank(size: ScreenSize) -> Self {
        let row = vec![Cell::default(); usize::from(size.cols)];
        Self {
            size,
            rows: vec![row; usize::from(size.rows)],
            cursor: Cursor::default(),
            modes: TerminalModes::default(),
            display_offset: 0,
            history: 0,
        }
    }

    /// The text of one row with trailing blanks removed (wide-char spacers
    /// skipped), as used by detection and `pane.read`.
    #[must_use]
    pub fn row_text(&self, row: usize) -> String {
        let Some(cells) = self.rows.get(row) else {
            return String::new();
        };
        let mut text: String = cells
            .iter()
            .filter(|c| c.width > 0)
            .map(Cell::text)
            .collect();
        text.truncate(text.trim_end().len());
        text
    }

    /// All rows as text, newline separated.
    #[must_use]
    pub fn text(&self) -> String {
        (0..self.rows.len())
            .map(|r| self.row_text(r))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_screen_has_requested_shape() {
        let s = Screen::blank(ScreenSize::new(4, 2));
        assert_eq!(s.rows.len(), 2);
        assert!(s.rows.iter().all(|r| r.len() == 4));
        assert_eq!(s.text(), "\n");
    }

    #[test]
    fn row_text_skips_spacers_and_trims() {
        let mut s = Screen::blank(ScreenSize::new(5, 1));
        s.rows[0][0] = Cell::new('界', 2);
        s.rows[0][1] = Cell::spacer();
        s.rows[0][2] = Cell::new('x', 1);
        assert_eq!(s.row_text(0), "界x");
        assert_eq!(s.row_text(9), "");
    }
}
