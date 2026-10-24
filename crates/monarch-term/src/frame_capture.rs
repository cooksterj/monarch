//! The rows copied out of a pane under its lock, for diffing outside it.

use monarch_proto::{Cell, PaneState, ScreenSize};

/// What [`crate::Terminal::capture`] copies while holding the pane lock:
/// only the damaged rows, so lock time scales with what changed rather
/// than with how much the program wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameCapture {
    /// Grid size at capture time.
    pub size: ScreenSize,
    /// Whether every row was copied (resize, scroll, forced).
    pub full: bool,
    /// Viewport rows copied, ascending.
    pub lines: Vec<u16>,
    /// Their contents, parallel to `lines`.
    pub rows: Vec<Vec<Cell>>,
    /// Cursor, modes and scroll position.
    pub state: PaneState,
}

impl FrameCapture {
    /// Pairs each copied line with its row.
    pub fn into_lines(self) -> impl Iterator<Item = (u16, Vec<Cell>)> {
        self.lines.into_iter().zip(self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_pair_with_rows() {
        let capture = FrameCapture {
            size: ScreenSize::new(2, 3),
            full: false,
            lines: vec![0, 2],
            rows: vec![vec![Cell::new('a', 1)], vec![Cell::new('c', 1)]],
            state: PaneState::default(),
        };
        let pairs: Vec<_> = capture.into_lines().map(|(l, r)| (l, r[0].ch)).collect();
        assert_eq!(pairs, [(0, 'a'), (2, 'c')]);
    }
}
