//! The help page (`prefix ?`): opens it from the keymap in use, takes the
//! keys while open and tells it how tall a page is.

use super::AttachApp;
use crate::client::AttachView;
use crate::{HelpState, HelpView, OverlayKey};

impl AttachApp {
    /// Opens the help for the current keys.
    pub(super) fn open_help(&mut self) {
        self.popups.help = Some(HelpState::new(&self.opts.keymap));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Keys while the help is open.
    pub(super) fn help_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(help) = self.popups.help.as_mut() else {
                break;
            };
            self.dirty = true;
            if !help.key(key) {
                self.popups.help = None;
                self.focus.set_overlay(self.any_overlay());
            }
        }
    }

    /// Tells the help how many lines fit before it is drawn.
    pub(super) fn size_help(&mut self) {
        if let Some(help) = self.popups.help.as_mut() {
            let panes = AttachView::pane_area(self.size, self.state.left());
            let area = ratatui::layout::Rect::new(0, 0, panes.cols, panes.rows);
            help.set_rows(HelpView::body_rows(area));
        }
    }
}
