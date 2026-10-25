//! What setup did to one event's Monarch hook.

/// The outcome for one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ChangeKind {
    /// Added a hook where there was none.
    Added,
    /// Fixed an existing hook's command, or dropped duplicates of it.
    Updated,
    /// Took Monarch's hook out.
    Removed,
    /// Left alone: the event's entry has a shape setup doesn't edit.
    Skipped,
    /// Already as wanted.
    Unchanged,
}

impl ChangeKind {
    /// How a report names it; `dry` says what would happen instead.
    #[must_use]
    pub fn verb(self, dry: bool) -> &'static str {
        match (self, dry) {
            (Self::Added, false) => "added",
            (Self::Added, true) => "would add",
            (Self::Updated, false) => "updated",
            (Self::Updated, true) => "would update",
            (Self::Removed, false) => "removed",
            (Self::Removed, true) => "would remove",
            (Self::Skipped, _) => "skipped (unrecognised format)",
            (Self::Unchanged, _) => "unchanged",
        }
    }

    /// Whether the file changes.
    #[must_use]
    pub fn edits(self) -> bool {
        matches!(self, Self::Added | Self::Updated | Self::Removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbs_read_as_plans_on_a_dry_run() {
        assert_eq!(ChangeKind::Added.verb(false), "added");
        assert_eq!(ChangeKind::Removed.verb(true), "would remove");
        assert_eq!(ChangeKind::Unchanged.verb(true), "unchanged");
        assert!(ChangeKind::Updated.edits());
        assert!(!ChangeKind::Skipped.edits());
    }
}
