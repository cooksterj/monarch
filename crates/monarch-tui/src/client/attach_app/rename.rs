//! The rename-tab prompt: asks for the shown tab's name, then tells the
//! server.

use monarch_proto::ClientMsg;

use super::AttachApp;
use crate::{OverlayKey, TabNameStep, TabNamer};

impl AttachApp {
    /// `prefix ,`: opens the prompt over the shown tab.
    pub(super) fn open_rename(&mut self) {
        let (_, active) = self.state.tab_labels();
        let Some(tab) = self.state.shown_tab() else {
            return;
        };
        let shown = self.state.tab_name(tab).trim();
        let namer = TabNamer::new(tab.focus, active + 1, tab.name.as_deref(), shown);
        self.popups.namer = Some(namer);
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Keys while the prompt is open.
    pub(super) fn rename_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(namer) = self.popups.namer.as_mut() else {
                break;
            };
            self.dirty = true;
            let pane = namer.pane();
            match namer.key(key) {
                TabNameStep::Open => continue,
                TabNameStep::Close => {}
                TabNameStep::Rename(name) => {
                    let _ = self.link.send(&ClientMsg::RenameTab { pane, name });
                }
            }
            self.popups.namer = None;
            self.focus.set_overlay(self.any_overlay());
        }
    }
}
