//! What a mouse event turns into.

use super::SelectEvent;

/// The outcome of routing one mouse event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseAction {
    /// Bytes for the focused pane (its own mouse encoding, or arrow keys).
    Forward(Vec<u8>),
    /// Scroll the focused pane's view: positive into history.
    Scroll(i32),
    /// A selection gesture for the main loop.
    Select(SelectEvent),
    /// Open the context menu at this host cell.
    Menu {
        /// Host column.
        col: u16,
        /// Host row.
        row: u16,
    },
    /// Nothing to do.
    Ignore,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_compare_by_value() {
        assert_eq!(MouseAction::Scroll(3), MouseAction::Scroll(3));
        assert_ne!(MouseAction::Scroll(3), MouseAction::Ignore);
    }
}
