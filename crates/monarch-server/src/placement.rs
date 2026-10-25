//! Where a newly started pane goes.

use monarch_core::SplitAxis;

/// A new pane's place in the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// A tab of its own; shown at once when `activate` (or when it is the
    /// first tab).
    Tab {
        /// Switch to the new tab.
        activate: bool,
    },
    /// Beside (or below) the focused pane of the shown tab, taking focus.
    Split(SplitAxis),
    /// Beside the focused pane of the shown tab, zoomed over it: when it
    /// exits, the tab looks as it did and focus goes back.
    Zoomed,
}

impl Placement {
    /// A tab in the background, as panes started with the server are.
    pub const BACKGROUND: Self = Self::Tab { activate: false };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_tabs_do_not_activate() {
        assert_eq!(Placement::BACKGROUND, Placement::Tab { activate: false });
        assert_ne!(Placement::Split(SplitAxis::Rows), Placement::BACKGROUND);
        assert_ne!(Placement::Zoomed, Placement::BACKGROUND);
    }
}
