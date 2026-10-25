//! What a key did to the open review pane.

/// The result of one key in the review pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStep {
    /// Still open (the file or scroll may have changed).
    Open,
    /// Closed.
    Close,
    /// Ask the server for the changes again.
    Refresh,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_ne!(ReviewStep::Open, ReviewStep::Refresh);
    }
}
