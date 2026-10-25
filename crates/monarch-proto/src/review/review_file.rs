//! One changed file and its diff.

use serde::{Deserialize, Serialize};

use super::{DiffLine, FileChange};

/// A changed file: its path in the checkout, how it changed, line counts
/// and the diff itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFile {
    /// Path from the checkout's top (the new path when renamed).
    pub path: String,
    /// How it changed.
    pub change: FileChange,
    /// Lines added.
    pub added: u32,
    /// Lines removed.
    pub removed: u32,
    /// The diff, hunk headers included.
    pub lines: Vec<DiffLine>,
}

impl ReviewFile {
    /// `path`, changed by `change`, with no lines yet.
    #[must_use]
    pub fn new(path: impl Into<String>, change: FileChange) -> Self {
        Self {
            path: path.into(),
            change,
            added: 0,
            removed: 0,
            lines: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_files_start_empty() {
        let file = ReviewFile::new("src/a.rs", FileChange::Untracked);
        assert_eq!((file.added, file.removed, file.lines.len()), (0, 0, 0));
        assert_eq!(file.path, "src/a.rs");
    }
}
