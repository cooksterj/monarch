//! What a key did in the project picker.

use monarch_core::PaneId;

/// The result of one key in the project picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectStep {
    /// Still open.
    Open,
    /// Closed.
    Close,
    /// Close and focus this pane, already working in the chosen worktree.
    Focus(PaneId),
    /// Close and open a tab in this worktree directory.
    OpenTab(String),
    /// Remove this worktree directory (confirmed); stay open.
    Remove(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_carry_their_target() {
        assert_ne!(
            ProjectStep::OpenTab("/a".into()),
            ProjectStep::Remove("/a".into())
        );
        assert_eq!(ProjectStep::Focus(PaneId(1)), ProjectStep::Focus(PaneId(1)));
    }
}
