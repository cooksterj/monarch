//! One worktree in the project picker.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// A worktree of a repository the session works in, and the panes there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeEntry {
    /// The repository's name (its main checkout's directory).
    pub project: String,
    /// The worktree's directory.
    pub path: String,
    /// The branch checked out there; `None` when detached.
    pub branch: Option<String>,
    /// Whether this is the repository's main checkout.
    pub main: bool,
    /// Panes working in it, in id order.
    pub panes: Vec<PaneId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survives_the_attach_stream_encoding() {
        let entry = WorktreeEntry {
            project: "app".into(),
            path: "/w/app/feat".into(),
            branch: Some("feat".into()),
            main: false,
            panes: vec![PaneId(2)],
        };
        let bytes = postcard::to_stdvec(&entry).expect("encode");
        assert_eq!(
            postcard::from_bytes::<WorktreeEntry>(&bytes).expect("decode"),
            entry
        );
    }
}
