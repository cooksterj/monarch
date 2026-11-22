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
    /// Open the file in the user's editor at `line` (from 1: the first
    /// selected line, or the cursor's); the viewer closes.
    Edit {
        /// The file, as the viewer names it.
        path: String,
        /// The line to open at.
        line: usize,
    },
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
        let edit = |line| FileViewerStep::Edit {
            path: "a".into(),
            line,
        };
        assert_ne!(edit(1), edit(2));
        assert_ne!(FileViewerStep::Open, FileViewerStep::Close);
    }
}
