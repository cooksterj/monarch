//! The rows of a pane that changed since the previous frame.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use super::{PaneState, RowDelta};

/// Changed rows plus current cursor/modes. Applies on top of the frame with
/// `seq - 1`; a client seeing a gap asks for a resync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    /// The pane.
    pub pane: PaneId,
    /// Layout generation; deltas from an older generation are discarded.
    pub generation: u32,
    /// Frame sequence number.
    pub seq: u64,
    /// Rows that changed.
    pub rows: Vec<RowDelta>,
    /// Cursor, modes, scroll position.
    pub state: PaneState,
}

impl Delta {
    /// Whether this delta follows a frame numbered `seq` in `generation`.
    #[must_use]
    pub fn follows(&self, generation: u32, seq: u64) -> bool {
        self.generation == generation && self.seq == seq + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_only_the_previous_frame_of_the_same_generation() {
        let delta = Delta {
            pane: PaneId(1),
            generation: 2,
            seq: 8,
            rows: Vec::new(),
            state: PaneState::default(),
        };
        assert!(delta.follows(2, 7));
        assert!(!delta.follows(2, 6), "gap");
        assert!(!delta.follows(1, 7), "stale generation");
    }
}
