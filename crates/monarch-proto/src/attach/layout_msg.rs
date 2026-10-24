//! The arrangement of panes the client should draw.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use super::PaneLayout;
use crate::ScreenSize;

/// Which panes are visible, where, and which has focus. Each new layout
/// bumps `generation`; frames from older generations are stale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutMsg {
    /// Layout generation.
    pub generation: u32,
    /// Size of the whole window the layout fills.
    pub size: ScreenSize,
    /// The pane receiving input.
    pub focus: Option<PaneId>,
    /// Visible panes.
    pub panes: Vec<PaneLayout>,
}

impl LayoutMsg {
    /// The layout entry of the focused pane.
    #[must_use]
    pub fn focused(&self) -> Option<&PaneLayout> {
        let focus = self.focus?;
        self.panes.iter().find(|p| p.pane == focus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_finds_the_focus_entry() {
        let pane = |id| PaneLayout {
            pane: PaneId(id),
            x: 0,
            y: 0,
            size: ScreenSize::new(10, 10),
            title: None,
        };
        let mut layout = LayoutMsg {
            generation: 1,
            size: ScreenSize::new(10, 10),
            focus: Some(PaneId(2)),
            panes: vec![pane(1), pane(2)],
        };
        assert_eq!(layout.focused().map(|p| p.pane), Some(PaneId(2)));
        layout.focus = Some(PaneId(9));
        assert!(layout.focused().is_none());
    }
}
