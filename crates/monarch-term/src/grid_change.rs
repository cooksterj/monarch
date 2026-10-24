//! What changed in a pane between two frames.

use monarch_proto::PaneState;

/// The result of [`crate::ShadowGrid::apply`]: which rows really changed
/// (damage over-reports) and the new pane state. Row contents are read from
/// the shadow grid, so encoding for several clients copies nothing extra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridChange {
    /// Whether the grid was resized: clients need a full snapshot.
    pub resized: bool,
    /// Viewport rows whose cells changed, ascending.
    pub lines: Vec<u16>,
    /// Current cursor, modes and scroll position.
    pub state: PaneState,
}

impl GridChange {
    /// Whether only the cursor, modes or scroll position changed.
    #[must_use]
    pub fn is_state_only(&self) -> bool {
        !self.resized && self.lines.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_only_means_no_rows_and_no_resize() {
        let mut change = GridChange {
            resized: false,
            lines: Vec::new(),
            state: PaneState::default(),
        };
        assert!(change.is_state_only());
        change.lines.push(1);
        assert!(!change.is_state_only());
    }
}
