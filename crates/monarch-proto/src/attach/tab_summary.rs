//! One tab as the tab bar and the go-to picker show it.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use super::PaneSummary;

/// A tab: every pane in it with its names, the focused one (whose name
/// names the tab), and whether that pane is zoomed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabSummary {
    /// The tab's focused pane.
    pub focus: PaneId,
    /// The focused pane fills the tab, hiding the others.
    pub zoomed: bool,
    /// Every pane in reading order.
    pub panes: Vec<PaneSummary>,
}

impl TabSummary {
    /// The focused pane's entry.
    #[must_use]
    pub fn focused(&self) -> Option<&PaneSummary> {
        self.panes.iter().find(|p| p.pane == self.focus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(id: u32, title: Option<&str>) -> PaneSummary {
        PaneSummary {
            pane: PaneId(id),
            title: title.map(Into::into),
            command: None,
            place: None,
        }
    }

    fn tab() -> TabSummary {
        TabSummary {
            focus: PaneId(3),
            zoomed: true,
            panes: vec![summary(2, None), summary(3, Some("vim"))],
        }
    }

    #[test]
    fn round_trips_through_postcard() {
        let bytes = postcard::to_allocvec(&tab()).expect("encode");
        assert_eq!(postcard::from_bytes::<TabSummary>(&bytes).ok(), Some(tab()));
    }

    #[test]
    fn finds_the_focused_pane() {
        assert_eq!(tab().focused().and_then(PaneSummary::name), Some("vim"));
        let missing = TabSummary {
            focus: PaneId(9),
            ..tab()
        };
        assert_eq!(missing.focused(), None);
    }
}
