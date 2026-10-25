//! Where a pane is working: its directory and the git checkout around it.

use serde::{Deserialize, Serialize};

use crate::CheckoutStatus;

/// A pane's working directory and, inside a git repository, which project,
/// checkout and branch it is. The sidebar groups panes by checkout.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PanePlace {
    /// The foreground program's working directory.
    pub cwd: String,
    /// The repository's name (`monarch`), shared by all its worktrees.
    pub project: Option<String>,
    /// The checkout's top directory; each worktree has its own.
    pub worktree: Option<String>,
    /// The checked-out branch, or a short commit id when detached.
    pub branch: Option<String>,
    /// Its uncommitted files and unpushed commits, when git could say.
    pub status: Option<CheckoutStatus>,
}

impl PanePlace {
    /// A directory outside any repository.
    #[must_use]
    pub fn outside(cwd: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_outside_has_no_repo() {
        let place = PanePlace {
            cwd: "/w/monarch/src".into(),
            project: Some("monarch".into()),
            worktree: Some("/w/monarch".into()),
            branch: Some("main".into()),
            status: Some(CheckoutStatus {
                changed: 2,
                ..CheckoutStatus::default()
            }),
        };
        let bytes = postcard::to_allocvec(&place).expect("encode");
        assert_eq!(postcard::from_bytes::<PanePlace>(&bytes).ok(), Some(place));
        let tmp = PanePlace::outside("/tmp");
        assert_eq!(
            (tmp.cwd.as_str(), tmp.project, tmp.worktree),
            ("/tmp", None, None)
        );
    }
}
