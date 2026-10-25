//! The part of a pane's viewport a selection covers.

use monarch_proto::{GridPoint, Screen, SelectShape};

/// An inclusive, normalised range of viewport cells. Rows may lie outside
/// the viewport (negative or past the bottom) when the selection extends
/// into scrollback; only the visible part is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionSpan {
    /// First row (viewport coordinates, may be negative).
    pub start_row: i64,
    /// Column on the first row.
    pub start_col: u16,
    /// Last row, inclusive.
    pub end_row: i64,
    /// Column on the last row, inclusive.
    pub end_col: u16,
    /// A rectangle: every row covers `start_col..=end_col`.
    pub block: bool,
}

impl SelectionSpan {
    /// The cells between grid points `a` and `b` (either order) in
    /// `shape`, in `screen`'s viewport coordinates.
    #[must_use]
    pub fn between(screen: &Screen, a: GridPoint, b: GridPoint, shape: SelectShape) -> Self {
        let (first, last) = (a.min(b), a.max(b));
        let (start_col, end_col) = match shape {
            SelectShape::Chars => (first.col, last.col),
            SelectShape::Lines => (0, u16::MAX),
            SelectShape::Block => (a.col.min(b.col), a.col.max(b.col)),
        };
        Self {
            start_row: first.viewport_row(screen),
            start_col,
            end_row: last.viewport_row(screen),
            end_col,
            block: shape == SelectShape::Block,
        }
    }

    /// Whether viewport cell (`row`, `col`) is selected.
    #[must_use]
    pub fn contains(&self, row: i64, col: u16) -> bool {
        if self.block {
            return (self.start_row..=self.end_row).contains(&row)
                && (self.start_col..=self.end_col).contains(&col);
        }
        let after_start = row > self.start_row || (row == self.start_row && col >= self.start_col);
        let before_end = row < self.end_row || (row == self.end_row && col <= self.end_col);
        after_start && before_end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_run_like_text_across_rows() {
        let span = SelectionSpan {
            start_row: 0,
            start_col: 5,
            end_row: 2,
            end_col: 1,
            block: false,
        };
        assert!(!span.contains(0, 4));
        assert!(span.contains(0, 5) && span.contains(0, 79));
        assert!(span.contains(1, 0));
        assert!(span.contains(2, 1) && !span.contains(2, 2));
        assert!(!span.contains(3, 0) && !span.contains(-1, 9));
    }

    #[test]
    fn shapes_cover_text_lines_or_a_rectangle() {
        use monarch_proto::ScreenSize;
        let screen = Screen::blank(ScreenSize::new(80, 5));
        let (a, b) = (GridPoint { line: 3, col: 9 }, GridPoint { line: 1, col: 2 });
        let chars = SelectionSpan::between(&screen, a, b, SelectShape::Chars);
        assert!(chars.contains(2, 50) && chars.contains(1, 2) && !chars.contains(3, 10));
        let lines = SelectionSpan::between(&screen, a, b, SelectShape::Lines);
        assert!(lines.contains(1, 0) && lines.contains(3, 79));
        let block = SelectionSpan::between(&screen, a, b, SelectShape::Block);
        assert!(block.contains(2, 5) && !block.contains(2, 10) && !block.contains(1, 1));
    }
}
