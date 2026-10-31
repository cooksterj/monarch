//! The rename prompts: ask for the shown tab's or the focused pane's
//! name, then tell the server.

use super::AttachApp;
use crate::{NamePrompt, NameStep, NameTarget, OverlayKey};

impl AttachApp {
    /// `prefix ,`: names the shown tab.
    pub(super) fn open_rename_tab(&mut self) {
        let (_, active) = self.state.tab_labels();
        let Some(tab) = self.state.shown_tab() else {
            return;
        };
        let shown = self.state.tab_name(tab).trim();
        let target = NameTarget::Tab(tab.focus);
        let title = format!("rename tab {}", active + 1);
        let prompt = NamePrompt::new(target, title, tab.name.as_deref(), shown);
        self.open_prompt(prompt);
    }

    /// `prefix .`: names the focused pane.
    pub(super) fn open_rename_pane(&mut self) {
        let Some(tab) = self.state.shown_tab() else {
            return;
        };
        let Some(pane) = tab.focused() else {
            return;
        };
        let shown = self.state.name(tab, pane).trim();
        let target = NameTarget::Pane(pane.pane);
        let given = pane.given_name.as_deref();
        let prompt = NamePrompt::new(target, "rename pane".into(), given, shown);
        self.open_prompt(prompt);
    }

    fn open_prompt(&mut self, prompt: NamePrompt) {
        self.popups.namer = Some(prompt);
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Keys while a prompt is open.
    pub(super) fn rename_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(prompt) = self.popups.namer.as_mut() else {
                break;
            };
            self.dirty = true;
            let target = prompt.target();
            match prompt.key(key) {
                NameStep::Open => continue,
                NameStep::Close => {}
                NameStep::Rename(name) => {
                    let _ = self.link.send(&target.rename(name));
                }
            }
            self.popups.namer = None;
            self.focus.set_overlay(self.any_overlay());
        }
    }
}
