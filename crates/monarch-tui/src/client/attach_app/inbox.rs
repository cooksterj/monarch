//! The attention inbox: every agent that needs the user, to jump to or
//! answer without leaving it; and `next-blocked`, which jumps straight to
//! the next agent waiting on a decision.

use std::time::Instant;

use monarch_core::{Action, age_label};
use monarch_proto::ClientMsg;

use super::AttachApp;
use super::select::NOTICE;
use crate::{Picker, PickerItem, QuickAnswer};

impl AttachApp {
    /// The inbox over the agents needing attention now.
    pub(super) fn inbox(&self) -> Picker {
        Picker::list("inbox", self.inbox_items())
    }

    /// One row per agent: glyph, tab, name and what it said; its state
    /// and how long it has been there (`blocked 3m`) at the right.
    fn inbox_items(&self) -> Vec<PickerItem> {
        self.state
            .needing_attention(Self::now())
            .into_iter()
            .map(|item| {
                let status = item.status;
                let said = status.activity.as_deref().unwrap_or(status.state.label());
                let state = status.state.label().to_lowercase();
                PickerItem {
                    label: format!(
                        "{} {} {} · {said}",
                        status.state.glyph(),
                        item.tab + 1,
                        item.name
                    ),
                    detail: match item.age {
                        Some(age) => format!("{state} {}", age_label(age)),
                        None => state,
                    },
                    action: Action::FocusPane(item.pane),
                }
            })
            .collect()
    }

    /// Keeps an open inbox in step with the agents.
    pub(super) fn refresh_inbox(&mut self) {
        if self.popups.picker.as_ref().is_some_and(Picker::is_list) {
            let items = self.inbox_items();
            if let Some(inbox) = self.popups.picker.as_mut() {
                inbox.refresh(items);
            }
        }
    }

    /// Answers the pane `row` is about with `key`: what `[agents.<id>]`
    /// maps it to for that pane's agent, else the key as typed.
    pub(super) fn answer(&mut self, row: Action, key: char) {
        let Action::FocusPane(pane) = row else {
            return;
        };
        let kind = self.state.agent_kind(pane);
        let answer = QuickAnswer::resolve(&self.opts.agents, kind, key);
        let bytes = answer.bytes.clone();
        let _ = self.link.send(&ClientMsg::Input { pane, bytes });
        self.notice = Some((answer.notice(pane), Instant::now() + NOTICE));
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
