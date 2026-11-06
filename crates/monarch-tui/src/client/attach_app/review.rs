//! The review pane: asks the server for the focused pane's changes, shows
//! them, takes the keys while open, and sends a note on a hunk to the
//! pane's agent.

use std::time::Instant;

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, FileChange, Review};

use super::AttachApp;
use super::select::NOTICE;
use crate::{NotePrompt, NoteStep, OverlayKey, ReviewState, ReviewStep};

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
                ReviewStep::Note => self.open_note(),
                ReviewStep::Close => self.close_review(),
            }
        }
    }

    /// `c` in the review: a prompt for a note on the hunk shown, for the
    /// reviewed pane's agent; the review stays open under it.
    fn open_note(&mut self) {
        let Some(state) = self.popups.review.as_ref() else {
            return;
        };
        let pane = state.pane();
        if !self.state.is_agent(pane) {
            let text = format!("pane {} runs no agent to send a note to", pane.0);
            self.notice = Some((text, Instant::now() + NOTICE));
            return;
        }
        let Some(note) = state.note_target() else {
            self.notice = Some(("nothing to note".into(), Instant::now() + NOTICE));
            return;
        };
        let agent = self
            .state
            .agent_kind(pane)
            .map_or("the agent", |k| k.name());
        self.popups.note = Some(NotePrompt::new(note, agent));
    }

    /// Keys while a note is being written.
    pub(super) fn note_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(prompt) = self.popups.note.as_mut() else {
                break;
            };
            self.dirty = true;
            match prompt.key(key) {
                NoteStep::Open => continue,
                NoteStep::Close => {}
                NoteStep::Send(text) => {
                    let note = prompt.note();
                    let msg = ClientMsg::Send {
                        pane: note.pane,
                        text: note.message(&text),
                        enter: true,
                    };
                    let _ = self.link.send(&msg);
                    let said = format!("note sent to pane {}", note.pane.0);
                    self.notice = Some((said, Instant::now() + NOTICE));
                }
            }
            self.popups.note = None;
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
