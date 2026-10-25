//! How a file changed.

use serde::{Deserialize, Serialize};

/// What happened to a file since the last commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FileChange {
    /// Edited.
    Modified,
    /// New and staged.
    Added,
    /// Deleted.
    Deleted,
    /// Moved (possibly edited too).
    Renamed,
    /// New and not yet added to git.
    Untracked,
}

impl FileChange {
    /// The one-letter code `git status` uses.
    #[must_use]
    pub fn letter(self) -> char {
        match self {
            Self::Modified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Untracked => '?',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_match_git_status() {
        assert_eq!(FileChange::Modified.letter(), 'M');
        assert_eq!(FileChange::Untracked.letter(), '?');
        assert_ne!(FileChange::Added, FileChange::Renamed);
    }
}
