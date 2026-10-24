//! Which way a split divides its space.

use serde::{Deserialize, Serialize};

/// How a split arranges its two children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    /// Side by side, divided by a vertical line (tmux `%`).
    Columns,
    /// Stacked, divided by a horizontal line (tmux `"`).
    Rows,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_by_name() {
        assert_eq!(
            serde_json::to_string(&SplitAxis::Rows).expect("json"),
            "\"rows\""
        );
    }
}
