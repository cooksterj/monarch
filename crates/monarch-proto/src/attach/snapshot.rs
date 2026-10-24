//! A pane's complete visible screen.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use super::{PaneState, Run};
use crate::ScreenSize;

/// Everything a client needs to draw a pane from scratch: sent when a pane
/// becomes visible, after a resize, and to resync a client that fell behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The pane.
    pub pane: PaneId,
    /// Layout generation this snapshot belongs to.
    pub generation: u32,
    /// Frame sequence number; deltas continue from it.
    pub seq: u64,
    /// Grid size.
    pub size: ScreenSize,
    /// Every row, top to bottom.
    pub rows: Vec<Vec<Run>>,
    /// Cursor, modes, scroll position.
    pub state: PaneState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_one_entry_per_row() {
        let snap = Snapshot {
            pane: PaneId(1),
            generation: 0,
            seq: 0,
            size: ScreenSize::new(10, 3),
            rows: vec![Vec::new(); 3],
            state: PaneState::default(),
        };
        assert_eq!(snap.rows.len(), usize::from(snap.size.rows));
    }
}
