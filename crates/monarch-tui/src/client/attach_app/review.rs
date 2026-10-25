//! The review pane: asks the server for the focused pane's changes, shows
//! them, and takes the keys while open.

use std::time::Instant;

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, FileChange, Review};

use super::AttachApp;
use super::select::NOTICE;
use crate::{OverlayKey, ReviewState, ReviewStep};

impl AttachApp {
    /// Opens the review of the focused pane's checkout; it shows
    /// "reading changes…" until the server answers.
    pub(super) fn open_review(&mut self) {
        let Some(pane) = self.state.focus() else {
            self.notice = Some(("no pane to review".into(), Instant::now() + NOTICE));
            self.dirty = true;
            return;
        };
        let _ = self.link.send(&ClientMsg::Review(pane));
        self.popups.review = Some(ReviewState::new(pane));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// The server's answer for `pane`, if the review is still waiting on
    /// it.
    pub(super) fn loaded_review(&mut self, pane: PaneId, review: Option<Review>) {
        if let Some(state) = self.popups.review.as_mut().filter(|s| s.pane() == pane) {
            state.load(review);
            self.dirty = true;
        }
    }

    /// Keys while the review is open.
    pub(super) fn review_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(state) = self.popups.review.as_mut() else {
                break;
            };
            self.dirty = true;
            match state.key(key) {
                ReviewStep::Open => {}
                ReviewStep::Refresh => {
                    let _ = self.link.send(&ClientMsg::Review(state.pane()));
                }
                ReviewStep::Edit => self.edit_selected(),
                ReviewStep::Close => self.close_review(),
            }
        }
    }

    /// Opens the selected file in the user's editor, zoomed over the
    /// focused pane, and closes the review.
    fn edit_selected(&mut self) {
        let Some(state) = self.popups.review.as_ref() else {
            return;
        };
        let Some(path) = state.editable() else {
            let deleted = state.selected().map(|f| f.change) == Some(FileChange::Deleted);
            let why = if deleted { "it was deleted" } else { "no file" };
            self.notice = Some((format!("nothing to open: {why}"), Instant::now() + NOTICE));
            return;
        };
        let msg = ClientMsg::Edit {
            pane: state.pane(),
            path: path.to_owned(),
            editor: editor(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok()),
        };
        let _ = self.link.send(&msg);
        self.close_review();
    }

    fn close_review(&mut self) {
        self.popups.review = None;
        self.focus.set_overlay(self.any_overlay());
    }
}

/// The editor to run: `$VISUAL`, else `$EDITOR`, else `vi`, as git picks.
fn editor(visual: Option<String>, editor: Option<String>) -> String {
    [visual, editor]
        .into_iter()
        .flatten()
        .find(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "vi".into())
}

#[cfg(test)]
mod tests {
    use super::editor;

    #[test]
    fn visual_then_editor_then_vi() {
        let some = |s: &str| Some(s.to_owned());
        assert_eq!(editor(some("code -w"), some("nano")), "code -w");
        assert_eq!(editor(some(" "), some("nano")), "nano");
        assert_eq!(editor(None, None), "vi");
    }
}
