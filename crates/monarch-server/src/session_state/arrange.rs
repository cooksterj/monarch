//! Keeping every pane sized to its place, and clients told about it.

use monarch_core::{ClientId, PaneId};
use monarch_proto::ScreenSize;

use super::SessionState;
use crate::ClientHandle;

impl SessionState {
    /// Focuses `id`, showing its tab. Unknown panes are ignored.
    pub fn set_focus(&mut self, id: PaneId) -> bool {
        let Some(index) = self.tabs.iter().position(|t| t.contains(id)) else {
            return false;
        };
        if self.focus() != Some(id) || self.active != index {
            let before = self.visible_panes();
            self.active = index;
            self.tabs[index].focus(id);
            self.arrange(&before);
        }
        true
    }

    /// A client reported its window size: recorded, and applied (the
    /// latest client wins, as tmux's `window-size latest`).
    pub fn resize_client(&mut self, id: ClientId, size: ScreenSize) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.set_window(size);
        }
        self.resize(size);
    }

    /// The size most recently reported by a client still attached.
    pub(super) fn latest_window(&self) -> Option<ScreenSize> {
        self.clients
            .values()
            .filter_map(ClientHandle::window)
            .max_by_key(|(at, _)| *at)
            .map(|(_, size)| size)
    }

    /// Resizes the window (latest client wins); every tab is laid out
    /// again so switching tabs later never reflows.
    pub fn resize(&mut self, size: ScreenSize) {
        if size != self.size {
            self.size = size;
            let before = self.visible_panes();
            self.arrange(&before);
        }
    }

    /// After any change to tabs, focus, zoom or size: gives each pane the
    /// size of its place, starts a new layout generation, and has clients
    /// fetch a snapshot of every pane that `before` did not show.
    pub(super) fn arrange(&mut self, before: &[PaneId]) {
        for (id, size) in self.pane_sizes() {
            if let Some(entry) = self.panes.get_mut(&id)
                && entry.size != size
            {
                entry.size = size;
                entry.io.resize(size);
                entry.io.shared().mark_dirty();
            }
        }
        self.generation += 1;
        for pane in self.visible_panes() {
            if !before.contains(&pane) {
                self.clients.values_mut().for_each(|c| c.mark_stale(pane));
            }
        }
    }

    /// Every pane's size in its own tab; a zoomed pane fills the window
    /// and the panes behind it keep their split sizes.
    fn pane_sizes(&self) -> Vec<(PaneId, ScreenSize)> {
        let area = self.area();
        self.tabs
            .iter()
            .flat_map(|tab| {
                let mut rects = tab.tree.layout(area);
                if tab.zoomed {
                    rects.retain(|(id, _)| *id != tab.focus);
                    rects.push((tab.focus, area));
                }
                rects
            })
            .map(|(id, r)| (id, ScreenSize::new(r.cols, r.rows)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc;

    use super::*;
    use crate::test_support::FakePane;

    #[test]
    fn when_a_client_leaves_the_window_fits_the_latest_one_left() {
        let mut s = SessionState::default();
        let pane = s.allocate_pane_id();
        s.insert_pane(pane, FakePane::boxed(ScreenSize::new(80, 24)));
        let (small_tx, _small_rx) = mpsc::channel(8);
        let (big_tx, _big_rx) = mpsc::channel(8);
        let small = s.add_client(small_tx);
        let big = s.add_client(big_tx);
        s.resize_client(small, ScreenSize::new(80, 23));
        std::thread::sleep(Duration::from_millis(2));
        s.resize_client(big, ScreenSize::new(150, 40));
        assert_eq!(s.size(), ScreenSize::new(150, 40), "the latest client wins");
        s.remove_client(big);
        assert_eq!(
            s.size(),
            ScreenSize::new(80, 23),
            "back to the client still attached"
        );
        s.remove_client(small);
        assert_eq!(s.size(), ScreenSize::new(80, 23), "no client: unchanged");
    }
}
