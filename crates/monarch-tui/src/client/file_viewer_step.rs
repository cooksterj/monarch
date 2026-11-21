//! What a key did in the file viewer.

use super::Quote;

/// The result of one key in the viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileViewerStep {
    /// Still open (the cursor, view or selection may have moved).
    Open,
    /// Closed.
    Close,
    /// Ask an agent about these lines; the viewer stays open under the
    /// composer.
    Ask(Quote),
    /// Copy this text to the clipboard.
    Copy(String),
    /// Open the file in the user's editor; the viewer closes.
    Edit(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_eq!(
            FileViewerStep::Copy("a".into()),
            FileViewerStep::Copy("a".into())
        );
        assert_ne!(FileViewerStep::Open, FileViewerStep::Close);
    }
}
