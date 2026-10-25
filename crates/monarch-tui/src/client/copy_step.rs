//! What a key did in copy mode.

use monarch_proto::{GridPoint, SelectShape};

/// The result of one copy-mode key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyStep {
    /// Still in copy mode (the cursor or selection may have moved).
    Stay,
    /// Leave copy mode and return to live output.
    Exit,
    /// Copy this range, then leave.
    Copy {
        /// First end of the range.
        start: GridPoint,
        /// Other end, inclusive.
        end: GridPoint,
        /// How the range becomes text.
        shape: SelectShape,
    },
    /// Ask the server where `query` appears next.
    Search {
        /// Plain text to find.
        query: String,
        /// Where to start (the cursor).
        from: GridPoint,
        /// Towards older output.
        backward: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_eq!(CopyStep::Stay, CopyStep::Stay);
        assert_ne!(CopyStep::Stay, CopyStep::Exit);
    }
}
