//! The project picker: asks the server for the session's worktrees, then
//! jumps to, opens or removes the one chosen.

use monarch_core::Action;
use monarch_proto::{ClientMsg, WorktreeEntry};

use super::AttachApp;
use crate::{OverlayKey, ProjectPicker, ProjectStep};

impl AttachApp {
    /// `prefix s`: opens the picker; it fills in when the server answers.
    pub(super) fn open_projects(&mut self) {
        let _ = self.link.send(&ClientMsg::Worktrees);
        self.popups.projects = Some(ProjectPicker::default());
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// The server's list, if the picker is still open.
    pub(super) fn loaded_worktrees(&mut self, entries: Vec<WorktreeEntry>) {
        let focus = self.state.focus();
        if let Some(projects) = self.popups.projects.as_mut() {
            projects.load(entries, focus);
            self.dirty = true;
        }
    }

    /// A removal finished: say so and refresh the list.
    pub(super) fn worktree_removed(&mut self, removed: Result<String, String>) {
        match removed {
            Ok(path) => {
                self.say(&format!("removed {path} (its branch is kept)"));
                if self.popups.projects.is_some() {
                    let _ = self.link.send(&ClientMsg::Worktrees);
                }
            }
            Err(why) => {
                if let Some(projects) = self.popups.projects.as_mut() {
                    projects.tell(&why);
                }
                self.say(&why);
            }
        }
    }

    /// Keys while the picker is open.
    pub(super) fn projects_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(projects) = self.popups.projects.as_mut() else {
                break;
            };
            self.dirty = true;
            let msg = match projects.key(key) {
                ProjectStep::Open => None,
                ProjectStep::Remove(path) => Some(ClientMsg::RemoveWorktree(path)),
                ProjectStep::Close => {
                    self.close_projects();
                    None
                }
                ProjectStep::Focus(pane) => {
                    self.close_projects();
                    Some(ClientMsg::Action(Action::FocusPane(pane)))
                }
                ProjectStep::OpenTab(path) => {
                    self.close_projects();
                    Some(ClientMsg::OpenWorktree(path))
                }
            };
            if let Some(msg) = msg {
                let _ = self.link.send(&msg);
            }
        }
    }

    fn close_projects(&mut self) {
        self.popups.projects = None;
        self.focus.set_overlay(self.any_overlay());
    }
}
