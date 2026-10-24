//! Identifies one frame of one pane.

use monarch_core::PaneId;

/// Which frame of which pane is being encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// The pane.
    pub pane: PaneId,
    /// Layout generation.
    pub generation: u32,
    /// Frame sequence number.
    pub seq: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_compare_by_value() {
        let a = FrameHeader {
            pane: PaneId(1),
            generation: 2,
            seq: 3,
        };
        assert_eq!(a, FrameHeader { ..a });
    }
}
