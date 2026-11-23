//! The new-worktree form: opens over the focused pane's repository, sends
//! the request, and shows how it went.

use std::time::Instant;

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, WorktreeCreated};

use super::AttachApp;
use super::select::NOTICE;
use crate::{OverlayKey, WorktreeForm, WorktreeStep};

impl AttachApp {
    /// `prefix N`: opens the form for the focused pane's repository.
    pub(super) fn open_worktree_form(&mut self) {
        let Some(dir) = self.state.focus().and_then(|p| self.repo_dir(p)) else {
            self.say("the focused pane is not in a git repository");
            return;
        };
        self.popups.form = Some(WorktreeForm::new(dir));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// The directory `pane` works in, if it is inside a git repository.
    pub(super) fn repo_dir(&self, pane: PaneId) -> Option<String> {
        self.state
            .place_of(pane)
            .filter(|p| p.worktree.is_some())
            .map(|p| p.cwd.clone())
    }

    /// Keys while the form is open.
    pub(super) fn worktree_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(form) = self.popups.form.as_mut() else {
                break;
            };
            self.dirty = true;
            match form.key(key) {
                WorktreeStep::Open => {}
                WorktreeStep::Submit => {
                    let root = self.opts.worktree_root.clone();
                    for request in form.requests(root) {
                        let _ = self.link.send(&ClientMsg::NewWorktree(request));
                    }
                }
                WorktreeStep::Close => {
                    self.popups.form = None;
                    self.focus.set_overlay(self.any_overlay());
                }
            }
        }
    }

    /// The server's answer: success closes the form once every worktree
    /// it asked for is made (the new tabs are already shown); a failure
    /// stays in the form to fix, or becomes a notice if the form was
    /// closed meanwhile.
    pub(super) fn worktree_made(&mut self, made: Result<WorktreeCreated, String>) {
        self.dirty = true;
        let pending = match (&made, self.popups.form.as_mut()) {
            (Ok(created), Some(form)) => !form.made(&created.branch),
            _ => false,
        };
        match (made, self.popups.form.as_mut()) {
            (Ok(_), Some(_)) if pending => {}
            (Ok(created), _) => {
                self.popups.form = None;
                self.focus.set_overlay(self.any_overlay());
                self.say(&format!("worktree {} ready", created.path));
            }
            (Err(why), Some(form)) => form.failed(why),
            (Err(why), None) => self.say(&why),
        }
    }

    /// Whether any pop-up or copy mode still takes the keys.
    pub(super) fn any_overlay(&self) -> bool {
        self.popups.any() || self.copy.is_some()
    }

    pub(super) fn say(&mut self, text: &str) {
        self.notice = Some((text.to_owned(), Instant::now() + NOTICE));
        self.dirty = true;
    }
}
