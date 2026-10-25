//! How an attach session ended.

/// Why [`super::AttachApp::attach`] returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachOutcome {
    /// The user detached (or the terminal closed); the session keeps running.
    Detached,
    /// The server went away (its last pane exited).
    ServerExited,
    /// The user chose another session: attach to it (starting it if new).
    Switch(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes_are_distinct() {
        assert_ne!(AttachOutcome::Detached, AttachOutcome::ServerExited);
        assert_ne!(AttachOutcome::Switch("a".into()), AttachOutcome::Detached);
    }
}
