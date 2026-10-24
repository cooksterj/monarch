//! Keeping every pane sized to its place, and clients told about it.

use monarch_core::PaneId;
use monarch_proto::ScreenSize;

use super::SessionState;

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
