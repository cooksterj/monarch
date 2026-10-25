//! The right-click menu in the client: opening it, and the clicks and
//! keys it takes while open.

use monarch_proto::ScreenSize;
use ratatui::layout::Rect;

use super::AttachApp;
use crate::client::{AttachOutcome, ContextMenu, PickerStep};
use crate::{ContextMenuView, HostInput, MouseButton, MouseExtractor, MouseKind, OverlayKey};

impl AttachApp {
    /// A right-click: the menu opens at the pointer.
    pub(super) fn open_menu(&mut self, col: u16, row: u16) {
        self.menu = Some(ContextMenu::new(&self.opts.keymap, col, row));
        self.selection = None;
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Host input while the menu is open: a click on a row runs it, a
    /// click elsewhere closes it, keys move and choose.
    pub(super) fn menu_input(&mut self, bytes: &[u8]) -> Option<AttachOutcome> {
        let mut pieces = Vec::new();
        MouseExtractor::new().split(bytes, &mut pieces);
        for piece in pieces {
            let Some(menu) = self.menu.as_mut() else {
                break;
            };
            let steps: Vec<PickerStep> = match piece {
                HostInput::Keys(keys) => OverlayKey::decode(&keys)
                    .into_iter()
                    .map(|k| menu.key(k))
                    .collect(),
                HostInput::Mouse(event) => match event.kind {
                    MouseKind::Press(MouseButton::Left | MouseButton::Right) => {
                        let area = host_area(self.size);
                        let row = ContextMenuView { menu }.item_at(area, event.col, event.row);
                        vec![menu.click(row)]
                    }
                    _ => Vec::new(),
                },
            };
            for step in steps {
                match step {
                    // The menu is no list: nothing to answer.
                    PickerStep::Open | PickerStep::Answer(..) => self.dirty = true,
                    PickerStep::Close => self.close_menu(),
                    PickerStep::Run(action) => {
                        self.close_menu();
                        return self.run_action(action);
                    }
                }
            }
        }
        None
    }

    fn close_menu(&mut self) {
        self.menu = None;
        self.focus.set_overlay(self.any_overlay());
        self.dirty = true;
    }
}

/// The whole host terminal, where the menu is placed.
pub(super) fn host_area(size: ScreenSize) -> Rect {
    Rect::new(0, 0, size.cols, size.rows)
}
