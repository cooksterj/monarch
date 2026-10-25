//! What a key did in the session picker.

/// The result of one key in the session picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStep {
    /// Still open.
    Open,
    /// Closed.
    Close,
    /// Close and attach to this session instead, starting it if new.
    Switch(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_carry_their_session() {
        assert_eq!(
            SessionStep::Switch("a".into()),
            SessionStep::Switch("a".into())
        );
        assert_ne!(SessionStep::Switch("a".into()), SessionStep::Close);
    }
}
