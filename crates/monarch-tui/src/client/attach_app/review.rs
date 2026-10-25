//! The review pane: asks the server for the focused pane's changes, shows
//! them, and takes the keys while open.

use std::time::Instant;

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, Review};

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
        self.review = Some(ReviewState::new(pane));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// The server's answer for `pane`, if the review is still waiting on
    /// it.
    pub(super) fn loaded_review(&mut self, pane: PaneId, review: Option<Review>) {
        if let Some(state) = self.review.as_mut().filter(|s| s.pane() == pane) {
            state.load(review);
            self.dirty = true;
        }
    }

    /// Keys while the review is open.
    pub(super) fn review_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(state) = self.review.as_mut() else {
                break;
            };
            self.dirty = true;
            match state.key(key) {
                ReviewStep::Open => {}
                ReviewStep::Refresh => {
                    let _ = self.link.send(&ClientMsg::Review(state.pane()));
                }
                ReviewStep::Close => {
                    self.review = None;
                    let others =
                        self.picker.is_some() || self.copy.is_some() || self.menu.is_some();
                    self.focus.set_overlay(others);
                }
            }
        }
    }
}
