//! One tab as the tab bar shows it.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// A tab: its focused pane (whose title names the tab) and whether that
/// pane is zoomed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabSummary {
    /// The tab's focused pane.
    pub focus: PaneId,
    /// That pane's title when the layout was sent; later changes arrive as
    /// `ServerMsg::Title`.
    pub title: Option<String>,
    /// The focused pane fills the tab, hiding the others.
    pub zoomed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_postcard() {
        let tab = TabSummary {
            focus: PaneId(3),
            title: Some("vim".into()),
            zoomed: true,
        };
        let bytes = postcard::to_allocvec(&tab).expect("encode");
        assert_eq!(postcard::from_bytes::<TabSummary>(&bytes).ok(), Some(tab));
    }
}
