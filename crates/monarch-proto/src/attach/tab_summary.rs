//! One tab as the tab bar and the go-to picker show it.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// A tab: every pane in it with its title, the focused one (whose title
/// names the tab), and whether that pane is zoomed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabSummary {
    /// The tab's focused pane.
    pub focus: PaneId,
    /// The focused pane fills the tab, hiding the others.
    pub zoomed: bool,
    /// Every pane in reading order, with its title when the layout was
    /// sent; later changes arrive as `ServerMsg::Title`.
    pub panes: Vec<(PaneId, Option<String>)>,
}

impl TabSummary {
    /// The focused pane's title when the layout was sent.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.panes
            .iter()
            .find(|(id, _)| *id == self.focus)
            .and_then(|(_, title)| title.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab() -> TabSummary {
        TabSummary {
            focus: PaneId(3),
            zoomed: true,
            panes: vec![(PaneId(2), None), (PaneId(3), Some("vim".into()))],
        }
    }

    #[test]
    fn round_trips_through_postcard() {
        let bytes = postcard::to_allocvec(&tab()).expect("encode");
        assert_eq!(postcard::from_bytes::<TabSummary>(&bytes).ok(), Some(tab()));
    }

    #[test]
    fn the_title_is_the_focused_panes() {
        assert_eq!(tab().title(), Some("vim"));
        let other = TabSummary {
            focus: PaneId(2),
            ..tab()
        };
        assert_eq!(other.title(), None);
        let missing = TabSummary {
            focus: PaneId(9),
            ..tab()
        };
        assert_eq!(missing.title(), None);
    }
}
