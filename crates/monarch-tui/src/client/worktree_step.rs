//! What a key did to the open new-worktree form.

/// The result of one key in the new-worktree form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeStep {
    /// Still open (the name or choice may have changed).
    Open,
    /// Ask the server to make the worktree.
    Submit,
    /// Closed.
    Close,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_ne!(WorktreeStep::Submit, WorktreeStep::Open);
    }
}
