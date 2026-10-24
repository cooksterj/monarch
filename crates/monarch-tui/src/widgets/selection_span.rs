//! The part of a pane's viewport a selection covers.

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
}

impl SelectionSpan {
    /// Whether viewport cell (`row`, `col`) is selected.
    #[must_use]
    pub fn contains(&self, row: i64, col: u16) -> bool {
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
        };
        assert!(!span.contains(0, 4));
        assert!(span.contains(0, 5) && span.contains(0, 79));
        assert!(span.contains(1, 0));
        assert!(span.contains(2, 1) && !span.contains(2, 2));
        assert!(!span.contains(3, 0) && !span.contains(-1, 9));
    }
}
