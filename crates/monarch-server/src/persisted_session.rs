//! A session as written to its state file.

use serde::{Deserialize, Serialize};

use crate::PersistedPane;

/// Panes in order and which one had focus.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedSession {
    /// Format version; files from a newer Monarch are not restored.
    pub version: u32,
    /// Panes in creation order.
    pub panes: Vec<PersistedPane>,
    /// Index into `panes` of the focused pane.
    pub focus: Option<usize>,
}

impl PersistedSession {
    /// The format this build writes.
    pub const VERSION: u32 = 1;

    /// A session of `panes` with `focus`, in the current format.
    #[must_use]
    pub fn new(panes: Vec<PersistedPane>, focus: Option<usize>) -> Self {
        Self {
            version: Self::VERSION,
            panes,
            focus,
        }
    }

    /// Whether this build can restore it.
    #[must_use]
    pub fn is_restorable(&self) -> bool {
        self.version == Self::VERSION && !self.panes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let session = PersistedSession::new(vec![PersistedPane::default()], Some(0));
        let json = serde_json::to_string(&session).expect("json");
        let back: PersistedSession = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, session);
        assert!(back.is_restorable());
    }

    #[test]
    fn newer_or_empty_sessions_are_not_restored() {
        let mut session = PersistedSession::new(vec![PersistedPane::default()], None);
        session.version += 1;
        assert!(!session.is_restorable());
        assert!(!PersistedSession::new(Vec::new(), None).is_restorable());
    }
}
