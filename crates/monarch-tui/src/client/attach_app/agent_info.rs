//! The agent info (`prefix i`): asks the server why the focused pane's
//! agent is in its state, shows the answer, and asks again whenever that
//! agent's status changes while it is open.

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, PaneExplanation};

use super::AttachApp;
use crate::client::AttachView;
use crate::{AgentInfo, AgentInfoView, InfoStep, OverlayKey};

impl AttachApp {
    /// Opens the info on the focused pane; it fills in when the server
    /// answers.
    pub(super) fn open_agent_info(&mut self) {
        let Some(pane) = self.state.focus() else {
            self.say("no pane is focused");
            return;
        };
        let name = self
            .state
            .all_panes()
            .into_iter()
            .find(|(_, p, _)| *p == pane)
            .map(|(_, _, name)| name.to_owned())
            .unwrap_or_default();
        self.popups.info = Some(AgentInfo::new(pane, name));
        let _ = self.link.send(&ClientMsg::Explain(pane));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// The server's answer about `pane`, if the info on it is still open.
    pub(super) fn explained(&mut self, pane: PaneId, explained: Option<Box<PaneExplanation>>) {
        if let Some(info) = self.popups.info.as_mut().filter(|i| i.pane() == pane) {
            info.load(explained);
            self.dirty = true;
        }
    }

    /// `pane`'s agent status changed: an open info on it asks again.
    pub(super) fn agent_changed(&mut self, pane: PaneId) {
        if self.popups.info.as_ref().is_some_and(|i| i.pane() == pane) {
            let _ = self.link.send(&ClientMsg::Explain(pane));
        }
    }

    /// Keys while the info is open.
    pub(super) fn info_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(info) = self.popups.info.as_mut() else {
                break;
            };
            self.dirty = true;
            match info.key(key) {
                InfoStep::Open => {}
                InfoStep::Refresh => {
                    let _ = self.link.send(&ClientMsg::Explain(info.pane()));
                }
                InfoStep::Close => {
                    self.popups.info = None;
                    self.focus.set_overlay(self.any_overlay());
                }
            }
        }
    }

    /// Tells the info how many lines fit before it is drawn.
    pub(super) fn size_info(&mut self) {
        let Some(info) = self.popups.info.as_mut() else {
            return;
        };
        let panes = AttachView::pane_area(self.size, self.state.left());
        let area = ratatui::layout::Rect::new(0, 0, panes.cols, panes.rows);
        let lines = info.lines(Self::now()).len();
        info.set_rows(AgentInfoView::body_rows(area, lines));
    }
}
