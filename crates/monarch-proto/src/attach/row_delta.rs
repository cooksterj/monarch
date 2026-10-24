//! One changed row inside a delta.

use serde::{Deserialize, Serialize};

use super::Run;

/// The new contents of viewport row `line`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowDelta {
    /// Viewport row index.
    pub line: u16,
    /// The whole row, as runs.
    pub runs: Vec<Run>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_row_has_no_runs() {
        let row = RowDelta {
            line: 4,
            runs: Vec::new(),
        };
        assert_eq!(row.line, 4);
        assert!(row.runs.is_empty());
    }
}
