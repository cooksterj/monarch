//! A text selection in one pane, anchored to grid lines (G10).
//!
//! Points are absolute lines, so the highlight follows its text while new
//! output scrolls it. The server reads the text for its bounds, so lines
//! that have scrolled out of view are copied too.

use monarch_proto::{Cell, GridPoint, Screen, SelectShape};

use super::SelectUnit;
use crate::SelectionSpan;

/// Characters that end a word for double-click selection, besides spaces.
/// Paths, URLs and `snake_case` stay whole.
const WORD_BREAKS: &str = "\"'`()[]{}<>|│,;";

/// Anchor (where the press was) and head (where the pointer is), each
/// already widened to the selection unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    anchor: (GridPoint, GridPoint),
    head: (GridPoint, GridPoint),
    unit: SelectUnit,
    moved: bool,
}

impl Selection {
    /// Starts at viewport cell (`row`, `col`).
    #[must_use]
    pub fn start(screen: &Screen, row: u16, col: u16, unit: SelectUnit) -> Self {
        let anchor = widen(screen, row, col, unit);
        Self {
            anchor,
            head: anchor,
            unit,
            moved: false,
        }
    }

    /// Moves the head to viewport cell (`row`, `col`).
    pub fn extend(&mut self, screen: &Screen, row: u16, col: u16) {
        self.head = widen(screen, row, col, self.unit);
        self.moved = true;
    }

    /// A single click that never moved selects nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.unit == SelectUnit::Cell && !self.moved
    }

    /// First and last selected points.
    #[must_use]
    pub fn bounds(&self) -> (GridPoint, GridPoint) {
        (
            self.anchor.0.min(self.head.0),
            self.anchor.1.max(self.head.1),
        )
    }

    /// The selection in `screen`'s viewport coordinates.
    #[must_use]
    pub fn span(&self, screen: &Screen) -> SelectionSpan {
        let (start, end) = self.bounds();
        SelectionSpan::between(screen, start, end, SelectShape::Chars)
    }
}

/// The range a press at (`row`, `col`) covers for `unit`.
fn widen(screen: &Screen, row: u16, col: u16, unit: SelectUnit) -> (GridPoint, GridPoint) {
    let at = |c: u16| GridPoint::at(screen, row, c);
    let cells = screen
        .rows
        .get(usize::from(row))
        .map_or(&[][..], Vec::as_slice);
    let last = u16::try_from(cells.len().saturating_sub(1)).unwrap_or(u16::MAX);
    match unit {
        SelectUnit::Cell => (at(col), at(col)),
        SelectUnit::Line => (at(0), at(last)),
        SelectUnit::Word => {
            let is_word = |i: usize| cells.get(i).is_some_and(word_char);
            let col = usize::from(col.min(last));
            if !is_word(col) {
                return (at(col as u16), at(col as u16));
            }
            let lo = (0..=col)
                .rev()
                .take_while(|i| is_word(*i))
                .last()
                .unwrap_or(col);
            let hi = (col..cells.len())
                .take_while(|i| is_word(*i))
                .last()
                .unwrap_or(col);
            (at(lo as u16), at(hi as u16))
        }
    }
}

/// Part of a word: printable, not blank and not a bracket or quote.
/// A wide character's spacer belongs to the character before it.
fn word_char(cell: &Cell) -> bool {
    cell.width == 0 || !(cell.ch.is_whitespace() || WORD_BREAKS.contains(cell.ch))
}

#[cfg(test)]
mod tests {
    use monarch_proto::ScreenSize;

    use super::*;

    fn screen(lines: &[&str]) -> Screen {
        let mut s = Screen::blank(ScreenSize::new(20, 4));
        for (r, line) in lines.iter().enumerate() {
            for (c, ch) in line.chars().enumerate() {
                s.rows[r][c] = Cell::new(ch, 1);
            }
        }
        s
    }

    fn cols(sel: &Selection) -> (u16, u16) {
        let (a, b) = sel.bounds();
        (a.col, b.col)
    }

    #[test]
    fn drag_selects_text_across_lines_in_either_direction() {
        let s = screen(&["hello world", "second line"]);
        let mut sel = Selection::start(&s, 0, 6, SelectUnit::Cell);
        assert!(sel.is_empty(), "a click is not a selection");
        sel.extend(&s, 1, 5);
        let expected = (GridPoint { line: 0, col: 6 }, GridPoint { line: 1, col: 5 });
        assert_eq!(sel.bounds(), expected);
        let mut back = Selection::start(&s, 1, 5, SelectUnit::Cell);
        back.extend(&s, 0, 6);
        assert_eq!(back.bounds(), expected);
    }

    #[test]
    fn double_click_selects_a_word_and_keeps_paths_whole() {
        let s = screen(&["cat (src/main.rs) x"]);
        let word = Selection::start(&s, 0, 8, SelectUnit::Word);
        assert!(!word.is_empty());
        assert_eq!(cols(&word), (5, 15), "src/main.rs");
        let paren = Selection::start(&s, 0, 4, SelectUnit::Word);
        assert_eq!(cols(&paren), (4, 4), "(");
    }

    #[test]
    fn triple_click_selects_whole_lines() {
        let s = screen(&["first", "second"]);
        let mut line = Selection::start(&s, 0, 2, SelectUnit::Line);
        assert_eq!(cols(&line), (0, 19), "the whole row");
        line.extend(&s, 1, 0);
        assert_eq!(line.bounds().1, GridPoint { line: 1, col: 19 });
    }

    #[test]
    fn the_highlight_follows_text_as_output_scrolls() {
        let mut s = screen(&["a", "b"]);
        let mut sel = Selection::start(&s, 1, 0, SelectUnit::Cell);
        sel.extend(&s, 1, 0);
        assert_eq!(sel.span(&s).start_row, 1);
        s.history += 1;
        assert_eq!(sel.span(&s).start_row, 0);
        s.history += 5;
        assert!(sel.span(&s).end_row < 0, "scrolled out of view");
        assert_eq!(sel.bounds().0.line, 1, "but still where its text is");
    }
}
