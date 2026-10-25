//! A worktree's uncommitted changes, for the review pane: what changed in
//! each file since the last commit, as diff lines.

mod diff_line;
mod diff_line_kind;
mod file_change;
mod review_file;

use serde::{Deserialize, Serialize};

pub use diff_line::DiffLine;
pub use diff_line_kind::DiffLineKind;
pub use file_change::FileChange;
pub use review_file::ReviewFile;

/// Every changed file in a checkout, staged or not, untracked included.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    /// The checkout's top directory.
    pub root: String,
    /// Its branch, or a short commit id when detached.
    pub branch: Option<String>,
    /// The changed files, in path order.
    pub files: Vec<ReviewFile>,
    /// Some diff lines were left out to keep the message small.
    pub truncated: bool,
}

impl Review {
    /// Lines added and removed across every file.
    #[must_use]
    pub fn totals(&self) -> (u32, u32) {
        self.files
            .iter()
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_add_up_and_round_trip() {
        let file = |added, removed| ReviewFile {
            added,
            removed,
            ..ReviewFile::new("a.rs", FileChange::Modified)
        };
        let review = Review {
            root: "/w".into(),
            branch: Some("main".into()),
            files: vec![file(3, 1), file(2, 0)],
            truncated: false,
        };
        assert_eq!(review.totals(), (5, 1));
        assert_eq!(Review::default().totals(), (0, 0));
        let bytes = postcard::to_stdvec(&review).expect("encode");
        assert_eq!(
            postcard::from_bytes::<Review>(&bytes).expect("decode"),
            review
        );
    }
}
