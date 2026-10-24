//! What applying server messages asks the client to do.

use monarch_core::PaneId;

/// Side effects collected by [`super::AttachState::apply`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Something visible changed.
    pub redraw: bool,
    /// Panes whose frames arrived out of sequence; ask for snapshots.
    pub resync: Vec<PaneId>,
    /// Bytes for the host terminal (bell, clipboard).
    pub host_write: Vec<u8>,
    /// Panes whose programs exited.
    pub exited: Vec<PaneId>,
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
