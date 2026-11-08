//! The launch picker: choose an agent, and the server starts it in a new
//! tab in the focused pane's directory.

use monarch_proto::ClientMsg;

use super::AttachApp;
use crate::{LaunchPicker, LaunchStep, OverlayKey};

impl AttachApp {
    /// `prefix A`: opens the picker over the configured agents.
    pub(super) fn open_launch(&mut self) {
        self.popups.launch = Some(LaunchPicker::new(&self.opts.agents));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Keys while the picker is open; choosing an agent asks the server
    /// for a tab running it.
    pub(super) fn launch_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(launch) = self.popups.launch.as_mut() else {
                break;
            };
            self.dirty = true;
            match launch.key(key) {
                LaunchStep::Open => {}
                LaunchStep::Close => self.close_launch(),
                LaunchStep::Launch(kind) => {
                    self.close_launch();
                    let _ = self.link.send(&ClientMsg::LaunchAgent(kind));
                }
            }
        }
    }

    fn close_launch(&mut self) {
        self.popups.launch = None;
        self.focus.set_overlay(self.any_overlay());
    }
}
