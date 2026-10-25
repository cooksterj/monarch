//! A session as written to its state file.

use monarch_core::Tab;
use serde::{Deserialize, Serialize};

use crate::PersistedPane;

/// Panes in order, how tabs arranged them and which one had focus.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedSession {
    /// Format version; files from a newer Monarch are not restored.
    pub version: u32,
    /// Panes in creation order.
    pub panes: Vec<PersistedPane>,
    /// Index into `panes` of the focused pane.
    pub focus: Option<usize>,
    /// The tabs: splits, ratios, focus and zoom, naming panes by their
    /// saved [`PersistedPane::id`]. Empty in version 1 files, whose panes
    /// come back one per tab.
    #[serde(default)]
    pub tabs: Vec<Tab>,
    /// Index into `tabs` of the one shown.
    #[serde(default)]
    pub active_tab: usize,
}

impl PersistedSession {
    /// The format this build writes.
    pub const VERSION: u32 = 2;

    /// A session of `panes` with `focus`, in the current format.
    #[must_use]
    pub fn new(panes: Vec<PersistedPane>, focus: Option<usize>) -> Self {
        Self {
            version: Self::VERSION,
            panes,
            focus,
            tabs: Vec::new(),
            active_tab: 0,
        }
    }

    /// The same session arranged in `tabs`, `active_tab` shown.
    #[must_use]
    pub fn with_tabs(self, tabs: Vec<Tab>, active_tab: usize) -> Self {
        Self {
            tabs,
            active_tab,
            ..self
        }
    }

    /// Whether this build can restore it: this format or an older one.
    #[must_use]
    pub fn is_restorable(&self) -> bool {
        (1..=Self::VERSION).contains(&self.version) && !self.panes.is_empty()
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

    #[test]
    fn version_one_files_still_restore_without_tabs() {
        let old = r#"{"version":1,"panes":[{"argv":[],"cwd":"/tmp"}],"focus":0}"#;
        let session: PersistedSession = serde_json::from_str(old).expect("parse");
        assert!(session.is_restorable());
        assert!(session.tabs.is_empty() && session.panes[0].id.is_none());
    }

    #[test]
    fn tabs_round_trip() {
        use monarch_core::{PaneId, SplitAxis};
        let mut tab = Tab::new(PaneId(4));
        tab.split(PaneId(7), SplitAxis::Rows);
        let session = PersistedSession::new(vec![PersistedPane::default()], None)
            .with_tabs(vec![tab, Tab::new(PaneId(9))], 1);
        let json = serde_json::to_string(&session).expect("json");
        assert_eq!(
            serde_json::from_str::<PersistedSession>(&json).ok(),
            Some(session)
        );
    }
}
