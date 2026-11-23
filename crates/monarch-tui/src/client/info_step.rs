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
    /// Still open: drop the agent's queued prompt at this index (0 is
    /// next).
    DropQueued(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_eq!(InfoStep::Open, InfoStep::Open);
        assert_ne!(InfoStep::Close, InfoStep::Refresh);
        assert_ne!(InfoStep::DropQueued(0), InfoStep::DropQueued(1));
    }
}
