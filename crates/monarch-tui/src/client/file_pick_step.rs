//! What a key did in the file picker.

/// The result of one key in the file picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilePickStep {
    /// Still open.
    Open,
    /// Closed without choosing.
    Close,
    /// View this file (its path from the checkout's top).
    Choose(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_eq!(
            FilePickStep::Choose("a".into()),
            FilePickStep::Choose("a".into())
        );
        assert_ne!(FilePickStep::Open, FilePickStep::Close);
    }
}
