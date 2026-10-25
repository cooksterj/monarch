//! Running actions, and the pop-ups that choose one: the command palette
//! and the go-to picker.

use monarch_core::Action;
use monarch_proto::ClientMsg;

use super::AttachApp;
use crate::client::AttachOutcome;
use crate::{OverlayKey, Picker, PickerItem, PickerStep};

impl AttachApp {
    /// Runs `action`: client actions here, the rest on the server.
    pub(super) fn run_action(&mut self, action: Action) -> Option<AttachOutcome> {
        match action {
            Action::Detach => {
                let _ = self.link.send(&ClientMsg::Detach);
                return Some(AttachOutcome::Detached);
            }
            Action::ToggleMouse => self.toggle_mouse(),
            Action::CommandPalette => self.open_picker(self.palette()),
            Action::GoToPane => self.open_picker(self.go_to()),
            Action::Inbox => self.open_picker(self.inbox()),
            Action::NextBlocked => self.next_blocked(),
            Action::Review => self.open_review(),
            Action::NewWorktree => self.open_worktree_form(),
            Action::Projects => self.open_projects(),
            Action::Sessions => self.open_sessions(),
            Action::CopyMode => self.open_copy(),
            Action::ToggleSidebar => self.toggle_sidebar(),
            Action::ReloadConfig => self.reload_config(),
            action => {
                let _ = self.link.send(&ClientMsg::Action(action));
            }
        }
        None
    }

    /// Feeds host input to the open pop-up or copy mode; keys after the
    /// one that closes it are dropped.
    pub(super) fn on_overlay_input(&mut self, bytes: &[u8]) -> Option<AttachOutcome> {
        if self.popups.menu.is_some() {
            return self.menu_input(bytes);
        }
        if self.popups.review.is_some() {
            self.review_input(bytes);
        } else if self.popups.form.is_some() {
            self.worktree_input(bytes);
        } else if self.popups.projects.is_some() {
            self.projects_input(bytes);
        } else if self.popups.sessions.is_some() {
            return self.sessions_input(bytes);
        } else if self.popups.picker.is_some() {
            return self.picker_input(bytes);
        } else {
            for key in OverlayKey::decode(bytes) {
                if self.copy.is_none() {
                    break;
                }
                self.copy_key(key);
            }
            if self.copy.is_none() {
                self.focus.set_overlay(false);
            }
        }
        None
    }

    /// Keys while the palette, go-to picker or inbox is open.
    fn picker_input(&mut self, bytes: &[u8]) -> Option<AttachOutcome> {
        for key in OverlayKey::decode(bytes) {
            let Some(picker) = self.popups.picker.as_mut() else {
                break;
            };
            self.dirty = true;
            match picker.key(key) {
                PickerStep::Open => {}
                PickerStep::Answer(row, key) => self.answer(row, key),
                PickerStep::Close => self.close_picker(),
                PickerStep::Run(action) => {
                    self.close_picker();
                    return self.run_action(action);
                }
            }
        }
        if self.popups.picker.is_none() && self.copy.is_none() {
            self.focus.set_overlay(false);
        }
        None
    }

    pub(super) fn open_picker(&mut self, picker: Picker) {
        self.popups.picker = Some(picker);
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    fn close_picker(&mut self) {
        self.popups.picker = None;
        self.focus.set_overlay(self.any_overlay());
        self.dirty = true;
    }

    /// Every action but the palette itself, then each tab, with its key.
    fn palette(&self) -> Picker {
        let keymap = &self.opts.keymap;
        let tabs = self.state.layout().map_or(0, |l| l.tabs.len());
        let tabs = (1..=tabs).filter_map(|n| u8::try_from(n).ok().map(Action::SelectTab));
        let items = Action::catalogue()
            .into_iter()
            .filter(|a| *a != Action::CommandPalette)
            .chain(tabs)
            .map(|a| {
                let key = keymap.key_for(a).map(|k| k.to_string());
                PickerItem::for_action(a, key.unwrap_or_default())
            })
            .collect();
        Picker::new("command palette", items)
    }

    /// Every pane in every tab, labelled `tab title`.
    fn go_to(&self) -> Picker {
        let focus = self.state.focus();
        let items = self
            .state
            .all_panes()
            .into_iter()
            .map(|(tab, pane, title)| {
                let title = title.trim();
                let label = if title.is_empty() {
                    format!("{} pane {}", tab + 1, pane.0)
                } else {
                    format!("{} {title}", tab + 1)
                };
                PickerItem {
                    label,
                    detail: if Some(pane) == focus { "here" } else { "" }.into(),
                    action: Action::FocusPane(pane),
                }
            })
            .collect();
        Picker::new("go to pane", items)
    }
}
