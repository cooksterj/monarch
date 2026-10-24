//! What pane threads report to the session's event loop.

use std::time::Instant;

use monarch_core::PaneId;

/// Low-volume pane happenings (output itself only sets the dirty flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneEvent {
    /// The program set or cleared its title.
    Title {
        /// The pane.
        pane: PaneId,
        /// New title.
        title: Option<String>,
    },
    /// The program rang the bell.
    Bell(PaneId),
    /// The program copied text to the clipboard (OSC 52).
    Clipboard {
        /// The pane.
        pane: PaneId,
        /// What it copied.
        text: String,
    },
    /// The program began a synchronized update that must be force-ended at
    /// `deadline` if it never finishes (the program died mid-frame).
    SyncPending {
        /// The pane.
        pane: PaneId,
        /// When to force it.
        deadline: Instant,
    },
    /// The program exited.
    Exited {
        /// The pane.
        pane: PaneId,
        /// Exit code, if it could be read.
        code: Option<u32>,
    },
}

impl PaneEvent {
    /// The pane this event is about.
    #[must_use]
    pub fn pane(&self) -> PaneId {
        match self {
            Self::Title { pane, .. }
            | Self::Bell(pane)
            | Self::Clipboard { pane, .. }
            | Self::SyncPending { pane, .. }
            | Self::Exited { pane, .. } => *pane,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_event_names_its_pane() {
        assert_eq!(PaneEvent::Bell(PaneId(3)).pane(), PaneId(3));
        let exit = PaneEvent::Exited {
            pane: PaneId(4),
            code: Some(0),
        };
        assert_eq!(exit.pane(), PaneId(4));
    }
}
