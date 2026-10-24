//! The last frame published for a pane, used to turn damage into deltas.

use monarch_proto::{Cell, PaneState, ScreenSize};

use crate::{FrameCapture, GridChange};

/// A copy of what clients were last sent for one pane. Owned by the frame
/// coalescer, never shared with the PTY reader, so diffing needs no lock.
#[derive(Debug, Clone)]
pub struct ShadowGrid {
    size: ScreenSize,
    rows: Vec<Vec<Cell>>,
    state: PaneState,
}

impl ShadowGrid {
    /// A blank grid.
    #[must_use]
    pub fn new(size: ScreenSize) -> Self {
        Self {
            size,
            rows: blank_rows(size),
            state: PaneState::default(),
        }
    }

    /// Folds a capture in, returning what really changed, or `None` if the
    /// capture changed nothing clients can see.
    pub fn apply(&mut self, capture: FrameCapture) -> Option<GridChange> {
        let resized = capture.size != self.size;
        if resized {
            self.size = capture.size;
            self.rows = blank_rows(capture.size);
        }
        let state_changed = capture.state != self.state;
        self.state = capture.state;
        let mut lines = Vec::new();
        for (line, row) in capture.into_lines() {
            let Some(slot) = self.rows.get_mut(usize::from(line)) else {
                continue;
            };
            if *slot != row {
                *slot = row;
                lines.push(line);
            }
        }
        (resized || state_changed || !lines.is_empty()).then_some(GridChange {
            resized,
            lines,
            state: self.state,
        })
    }

    /// Grid size.
    #[must_use]
    pub fn size(&self) -> ScreenSize {
        self.size
    }

    /// Every row, top to bottom (for snapshots).
    #[must_use]
    pub fn rows(&self) -> &[Vec<Cell>] {
        &self.rows
    }

    /// One row, if it exists.
    #[must_use]
    pub fn row(&self, line: u16) -> Option<&[Cell]> {
        self.rows.get(usize::from(line)).map(Vec::as_slice)
    }

    /// Current cursor, modes and scroll position.
    #[must_use]
    pub fn state(&self) -> PaneState {
        self.state
    }
}

fn blank_rows(size: ScreenSize) -> Vec<Vec<Cell>> {
    vec![vec![Cell::default(); usize::from(size.cols)]; usize::from(size.rows)]
}

#[cfg(test)]
mod tests;
