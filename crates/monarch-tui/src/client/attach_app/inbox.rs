//! The attention inbox: every agent that needs the user, to jump to or
//! answer without leaving it; and `next-blocked`, which jumps straight to
//! the next agent waiting on a decision.

use std::time::Instant;

use monarch_core::Action;
use monarch_proto::ClientMsg;

use super::AttachApp;
use super::select::NOTICE;
use crate::{Picker, PickerItem};

impl AttachApp {
    /// The inbox over the agents needing attention now.
    pub(super) fn inbox(&self) -> Picker {
        Picker::list("inbox", self.inbox_items())
    }

    /// One row per agent: glyph, tab, name and what it said; its state at
    /// the right.
    fn inbox_items(&self) -> Vec<PickerItem> {
        self.state
            .needing_attention()
            .into_iter()
            .map(|(tab, pane, name, status)| {
                let said = status.activity.as_deref().unwrap_or(status.state.label());
                PickerItem {
                    label: format!("{} {} {name} · {said}", status.state.glyph(), tab + 1),
                    detail: status.state.label().to_lowercase(),
                    action: Action::FocusPane(pane),
                }
            })
            .collect()
    }

    /// Keeps an open inbox in step with the agents.
    pub(super) fn refresh_inbox(&mut self) {
        if self.picker.as_ref().is_some_and(Picker::is_list) {
            let items = self.inbox_items();
            if let Some(inbox) = self.picker.as_mut() {
                inbox.refresh(items);
            }
        }
    }

    /// Sends `key` to the pane `row` is about, as if typed there.
    pub(super) fn answer(&mut self, row: Action, key: char) {
        let Action::FocusPane(pane) = row else {
            return;
        };
        let bytes = key.to_string().into_bytes();
        let _ = self.link.send(&ClientMsg::Input { pane, bytes });
        self.notice = Some((
            format!("sent {key} to pane {}", pane.0),
            Instant::now() + NOTICE,
        ));
        self.dirty = true;
    }

    /// Focuses the next blocked agent, or says there is none.
    pub(super) fn next_blocked(&mut self) {
        match self.state.next_blocked(self.state.focus()) {
            Some(pane) => {
                let _ = self.link.send(&ClientMsg::Action(Action::FocusPane(pane)));
            }
            None => {
                self.notice = Some(("no agent is blocked".into(), Instant::now() + NOTICE));
                self.dirty = true;
            }
        }
    }
}
