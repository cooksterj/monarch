//! What a key did to the open agent info.

/// The result of one key in the agent info.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoStep {
    /// Still open (it may have scrolled).
    Open,
    /// Closed.
    Close,
    /// Still open: ask the server again.
    Refresh,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_eq!(InfoStep::Open, InfoStep::Open);
        assert_ne!(InfoStep::Close, InfoStep::Refresh);
    }
}
