//! What applying server messages asks the client to do.

use monarch_core::PaneId;
use monarch_proto::{GridPoint, Review};

/// Side effects collected by [`super::AttachState::apply`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Something visible changed.
    pub redraw: bool,
    /// Panes whose frames arrived out of sequence; ask for snapshots.
    pub resync: Vec<PaneId>,
    /// Bytes for the host terminal (bell).
    pub host_write: Vec<u8>,
    /// Text a pane copied; goes to the user's clipboard.
    pub clipboard: Vec<String>,
    /// Panes whose programs exited.
    pub exited: Vec<PaneId>,
    /// Answers to searches: the pane and where the match is, if anywhere.
    pub found: Vec<(PaneId, Option<(GridPoint, GridPoint)>)>,
    /// Answers to review requests: the pane and its checkout's changes.
    pub reviews: Vec<(PaneId, Option<Review>)>,
}

impl Applied {
    /// Whether there is nothing to do.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert!(Applied::default().is_empty());
        let redraw = Applied {
            redraw: true,
            ..Applied::default()
        };
        assert!(!redraw.is_empty());
    }
}
