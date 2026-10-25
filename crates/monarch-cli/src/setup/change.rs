//! What setup did to one event.

use super::ChangeKind;

/// An event and what happened to its Monarch hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Change {
    /// The hook event, e.g. `Stop`.
    pub event: String,
    /// What happened.
    pub kind: ChangeKind,
}

impl Change {
    /// `kind` happened to `event`.
    pub fn new(event: impl Into<String>, kind: ChangeKind) -> Self {
        Self {
            event: event.into(),
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_name_their_event() {
        let change = Change::new("Stop", ChangeKind::Added);
        assert_eq!(change.event, "Stop");
        assert_ne!(change, Change::new("Stop", ChangeKind::Removed));
    }
}
