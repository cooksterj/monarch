//! The answer to `worktree.create`.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// Where the new worktree is and the pane opened in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeCreated {
    /// The worktree's directory.
    pub path: String,
    /// The branch checked out there.
    pub branch: String,
    /// Whether the branch was made for it (else it already existed).
    pub new_branch: bool,
    /// The pane opened in it.
    pub pane: PaneId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_as_json() {
        let created = WorktreeCreated {
            path: "/r-worktrees/feat".into(),
            branch: "feat".into(),
            new_branch: true,
            pane: PaneId(3),
        };
        let json = serde_json::to_string(&created).expect("json");
        assert_eq!(
            serde_json::from_str::<WorktreeCreated>(&json).expect("back"),
            created
        );
    }
}
