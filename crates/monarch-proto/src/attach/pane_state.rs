//! Per-frame pane facts that are not cell contents.

use serde::{Deserialize, Serialize};

use crate::{Cursor, TerminalModes};

/// Cursor, modes and scroll position: sent whole with every snapshot and
/// delta, since they are small and change often.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PaneState {
    /// Cursor position and appearance.
    pub cursor: Cursor,
    /// Modes the client mirrors to the host.
    pub modes: TerminalModes,
    /// Lines scrolled back into history (0 = live view).
    pub display_offset: u32,
    /// Lines of scrollback available.
    pub history: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_live_view() {
        let state = PaneState::default();
        assert_eq!(state.display_offset, 0);
        assert!(state.cursor.visible);
    }
}
