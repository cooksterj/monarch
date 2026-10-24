//! Session commands from clients: splits, focus, zoom, tabs.

use monarch_core::{Action, RESIZE_STEP};

use super::SessionState;
use crate::Placement;

/// What the caller still has to do after [`SessionState::apply_action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionEffect {
    /// Nothing; the session changed (or the action did not apply).
    Done,
    /// Start a shell pane here (needs a PTY, so it happens outside).
    Spawn(Placement),
}

impl SessionState {
    /// Applies `action` to the session.
    pub fn apply_action(&mut self, action: Action) -> ActionEffect {
        match action {
            Action::ClosePane => self.close_focused(),
            Action::FocusPane(id) => {
                self.set_focus(id);
            }
            _ => return self.apply_to_tab(action),
        }
        ActionEffect::Done
    }

    /// Applies `action` to the shown tab.
    fn apply_to_tab(&mut self, action: Action) -> ActionEffect {
        let before = self.visible_panes();
        let area = self.area();
        let tabs = self.tabs.len();
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return ActionEffect::Done;
        };
        let changed = match action {
            Action::Split(axis) => return ActionEffect::Spawn(Placement::Split(axis)),
            Action::NewTab => return ActionEffect::Spawn(Placement::Tab { activate: true }),
            Action::Focus(dir) => tab.focus_towards(area, dir),
            Action::FocusNext => tab.focus_cycle(true),
            Action::FocusPrev => tab.focus_cycle(false),
            Action::Swap(dir) => tab.swap_towards(area, dir),
            Action::SwapNext => tab.swap_cycle(true),
            Action::SwapPrev => tab.swap_cycle(false),
            Action::Resize(dir) => {
                !tab.zoomed && tab.tree.resize(area, tab.focus, dir, RESIZE_STEP)
            }
            Action::ToggleZoom => {
                let many = tab.tree.panes().len() > 1;
                tab.zoomed = many && !tab.zoomed;
                many
            }
            Action::Equalize => {
                tab.tree.equalize();
                true
            }
            Action::NextTab => self.show_tab((self.active + 1) % tabs),
            Action::PrevTab => self.show_tab((self.active + tabs - 1) % tabs),
            Action::SelectTab(n) => self.show_tab(usize::from(n).wrapping_sub(1)),
            _ => false,
        };
        if changed {
            self.arrange(&before);
        }
        ActionEffect::Done
    }

    /// Ends the focused pane's program; the pane goes when it exits, like
    /// any other.
    fn close_focused(&self) {
        if let Some(entry) = self.focus().and_then(|id| self.panes.get(&id)) {
            entry.io.close();
        }
    }

    /// Shows tab `index`; returns whether that changed anything.
    fn show_tab(&mut self, index: usize) -> bool {
        let changed = index < self.tabs.len() && index != self.active;
        if changed {
            self.active = index;
        }
        changed
    }
}
