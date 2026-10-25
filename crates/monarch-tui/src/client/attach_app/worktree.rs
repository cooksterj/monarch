//! The new-worktree form: opens over the focused pane's repository, sends
//! the request, and shows how it went.

use std::time::Instant;

use monarch_proto::{ClientMsg, WorktreeCreated};

use super::AttachApp;
use super::select::NOTICE;
use crate::{OverlayKey, WorktreeForm, WorktreeStep};

impl AttachApp {
    /// `prefix N`: opens the form for the focused pane's repository.
    pub(super) fn open_worktree_form(&mut self) {
        let place = self.state.focus().and_then(|p| self.state.place_of(p));
        let Some(dir) = place
            .filter(|p| p.worktree.is_some())
            .map(|p| p.cwd.clone())
        else {
            self.say("the focused pane is not in a git repository");
            return;
        };
        self.popups.form = Some(WorktreeForm::new(dir));
        self.focus.set_overlay(true);
        self.dirty = true;
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
                    let _ = self.link.send(&ClientMsg::NewWorktree(form.request(root)));
                }
                WorktreeStep::Close => {
                    self.popups.form = None;
                    self.focus.set_overlay(self.any_overlay());
                }
            }
        }
    }

    /// The server's answer: success closes the form (the new tab is
    /// already shown); a failure stays in the form to fix, or becomes a
    /// notice if the form was closed meanwhile.
    pub(super) fn worktree_made(&mut self, made: Result<WorktreeCreated, String>) {
        self.dirty = true;
        match (made, self.popups.form.as_mut()) {
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
