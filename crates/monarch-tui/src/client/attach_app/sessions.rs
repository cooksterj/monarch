//! The session picker: lists the running sessions off this thread, then
//! switches to the one chosen (or a new one by name).

use std::sync::Arc;

use super::AttachApp;
use crate::client::AttachOutcome;
use crate::{ClientEvent, OverlayKey, SessionEntry, SessionPicker, SessionStep};

impl AttachApp {
    /// `prefix S`: opens the picker; it fills in when the list arrives.
    pub(super) fn open_sessions(&mut self) {
        let lister = Arc::clone(&self.lister);
        let post = self.post.clone();
        std::thread::spawn(move || {
            let _ = post.send(ClientEvent::Sessions(lister.list()));
        });
        self.popups.sessions = Some(SessionPicker::new(self.opts.session.clone()));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// The list, if the picker is still open.
    pub(super) fn loaded_sessions(&mut self, list: Vec<SessionEntry>) {
        if let Some(sessions) = self.popups.sessions.as_mut() {
            sessions.load(list);
            self.dirty = true;
        }
    }

    /// Keys while the picker is open; choosing a session ends this attach.
    pub(super) fn sessions_input(&mut self, bytes: &[u8]) -> Option<AttachOutcome> {
        for key in OverlayKey::decode(bytes) {
            let sessions = self.popups.sessions.as_mut()?;
            self.dirty = true;
            match sessions.key(key) {
                SessionStep::Open => {}
                SessionStep::Close => self.close_sessions(),
                SessionStep::Switch(name) => {
                    self.close_sessions();
                    return Some(AttachOutcome::Switch(name));
                }
            }
        }
        None
    }

    fn close_sessions(&mut self) {
        self.popups.sessions = None;
        self.focus.set_overlay(self.any_overlay());
    }
}
