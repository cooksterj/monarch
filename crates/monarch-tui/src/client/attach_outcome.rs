//! How an attach session ended.

/// Why [`super::AttachApp::attach`] returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachOutcome {
    /// The user detached (or the terminal closed); the session keeps running.
    Detached,
    /// The server went away (its last pane exited).
    ServerExited,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes_are_distinct() {
        assert_ne!(AttachOutcome::Detached, AttachOutcome::ServerExited);
    }
}
