//! The right-click menu: a short list of pane and tab commands opened at
//! the pointer, chosen with a click or the keyboard.

use monarch_core::{Action, Keymap, SplitAxis};

use super::{PickerItem, PickerStep};
use crate::OverlayKey;

/// What the menu offers, in order.
const ACTIONS: [Action; 8] = [
    Action::Split(SplitAxis::Columns),
    Action::Split(SplitAxis::Rows),
    Action::ToggleZoom,
    Action::ClosePane,
    Action::NewTab,
    Action::CopyMode,
    Action::ToggleSidebar,
    Action::CommandPalette,
];

/// The rows, the highlighted one and the host cell it was opened at.
/// Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenu {
    items: Vec<PickerItem>,
    selected: usize,
    at: (u16, u16),
}

impl ContextMenu {
    /// The menu at host cell (`col`, `row`), each row showing its key.
    #[must_use]
    pub fn new(keymap: &Keymap, col: u16, row: u16) -> Self {
        let items = ACTIONS
            .into_iter()
            .map(|a| {
                let key = keymap.key_for(a).map(|k| k.to_string());
                PickerItem::for_action(a, key.unwrap_or_default())
            })
            .collect();
        Self {
            items,
            selected: 0,
            at: (col, row),
        }
    }

    /// Up/Down (and Ctrl-p/n, `k`/`j`) move, Enter runs, Esc/`q`/Ctrl-c
    /// close.
    pub fn key(&mut self, key: OverlayKey) -> PickerStep {
        let last = self.items.len().saturating_sub(1);
        match key {
            OverlayKey::Up | OverlayKey::Ctrl('p') | OverlayKey::Char('k') => {
                self.selected = if self.selected == 0 {
                    last
                } else {
                    self.selected - 1
                };
            }
            OverlayKey::Down | OverlayKey::Ctrl('n') | OverlayKey::Char('j') => {
                self.selected = if self.selected >= last {
                    0
                } else {
                    self.selected + 1
                };
            }
            OverlayKey::Enter => return self.run(self.selected),
            OverlayKey::Esc | OverlayKey::Ctrl('c' | 'g') | OverlayKey::Char('q') => {
                return PickerStep::Close;
            }
            _ => {}
        }
        PickerStep::Open
    }

    /// A click on row `index` runs it; a click anywhere else (`None`)
    /// closes the menu.
    pub fn click(&mut self, index: Option<usize>) -> PickerStep {
        match index {
            Some(i) if i < self.items.len() => self.run(i),
            _ => PickerStep::Close,
        }
    }

    fn run(&self, index: usize) -> PickerStep {
        self.items
            .get(index)
            .map_or(PickerStep::Close, |item| PickerStep::Run(item.action))
    }

    /// The rows.
    #[must_use]
    pub fn items(&self) -> &[PickerItem] {
        &self.items
    }

    /// The highlighted row.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The host cell it was opened at.
    #[must_use]
    pub fn at(&self) -> (u16, u16) {
        self.at
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::Key;

    use super::*;

    #[test]
    fn rows_show_their_keys_and_the_keyboard_moves_and_runs() {
        let mut menu = ContextMenu::new(&Keymap::default(), 10, 4);
        assert_eq!(menu.at(), (10, 4));
        assert_eq!(menu.items()[0].label, "split right");
        assert_eq!(menu.items()[2].detail, Key::Char('z').to_string());
        assert_eq!(menu.key(OverlayKey::Up), PickerStep::Open);
        assert_eq!(menu.selected(), ACTIONS.len() - 1, "wraps to the bottom");
        menu.key(OverlayKey::Down);
        menu.key(OverlayKey::Char('j'));
        assert_eq!(menu.key(OverlayKey::Enter), PickerStep::Run(ACTIONS[1]));
        assert_eq!(menu.key(OverlayKey::Esc), PickerStep::Close);
        assert_eq!(menu.key(OverlayKey::Char('x')), PickerStep::Open, "ignored");
    }

    #[test]
    fn clicks_run_a_row_or_close() {
        let mut menu = ContextMenu::new(&Keymap::default(), 0, 0);
        assert_eq!(menu.click(Some(3)), PickerStep::Run(Action::ClosePane));
        assert_eq!(menu.click(None), PickerStep::Close);
        assert_eq!(menu.click(Some(99)), PickerStep::Close);
    }
}
