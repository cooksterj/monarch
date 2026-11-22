//! The ask composer in the client: opening it on the selection, a
//! copy-mode range or a review hunk, its keys, and sending the prompt to
//! the chosen agent.

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, GridPoint, SelectShape};

use super::AttachApp;
use crate::{AskPrompt, AskStep, AskTarget, OverlayKey, Quote};

impl AttachApp {
    /// `prefix e`: asks about the mouse selection in the focused pane,
    /// once the server has read its text, or opens the composer without
    /// a quote when nothing is selected.
    pub(super) fn open_ask(&mut self) {
        let pane = self.state.focus();
        match (pane, self.selection.take().filter(|s| !s.is_empty())) {
            (Some(pane), Some(selection)) => {
                let (start, end) = selection.bounds();
                self.request_quote(pane, start, end, SelectShape::Chars);
                self.dirty = true;
            }
            _ => self.open_ask_with(None),
        }
    }

    /// Asks the server for the text of a range; the composer opens when it
    /// arrives ([`Self::quoted`]).
    pub(super) fn request_quote(
        &mut self,
        pane: PaneId,
        start: GridPoint,
        end: GridPoint,
        shape: SelectShape,
    ) {
        self.popups.quoting = Some(pane);
        let msg = ClientMsg::Quote {
            pane,
            start,
            end,
            shape,
        };
        let _ = self.link.send(&msg);
    }

    /// The text of a range asked for: opens the composer quoting it.
    pub(super) fn quoted(&mut self, pane: PaneId, text: String) {
        if self.popups.quoting.take_if(|p| *p == pane).is_some() {
            self.open_ask_with(Some(Quote::Screen { pane, text }));
        }
    }

    /// Opens the composer quoting `quote`, over whatever is open: to the
    /// agents running, or new ones it starts.
    pub(super) fn open_ask_with(&mut self, quote: Option<Quote>) {
        let targets = self.ask_targets(quote.as_ref().map(Quote::pane));
        self.popups.ask = Some(AskPrompt::new(quote, targets));
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Keys while the composer is open.
    pub(super) fn ask_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(prompt) = self.popups.ask.as_mut() else {
                break;
            };
            self.dirty = true;
            match prompt.key(key) {
                AskStep::Open => {}
                AskStep::Refuse(why) => self.say(&why),
                AskStep::Close => self.close_ask(),
                AskStep::Queue { pane, text } => {
                    let _ = self.link.send(&ClientMsg::SendWhenReady { pane, text });
                    self.close_ask();
                    self.say(&format!(
                        "queued for the agent in pane {}: sent when it is at its prompt",
                        pane.0
                    ));
                }
                AskStep::Start { kinds, text } => {
                    let names: Vec<&str> = kinds.iter().map(|k| k.name()).collect();
                    let msg = ClientMsg::LaunchWithPrompt {
                        kinds,
                        prompt: text,
                    };
                    let _ = self.link.send(&msg);
                    self.close_ask();
                    self.say(&format!("starting {} with the ask", names.join(" and ")));
                }
                AskStep::Send { pane, text } => {
                    let _ = self.link.send(&ClientMsg::Send {
                        pane,
                        text,
                        enter: true,
                    });
                    self.close_ask();
                    self.say(&format!("asked the agent in pane {}", pane.0));
                }
            }
        }
    }

    /// Keeps an open composer's agents in step with their states.
    pub(super) fn refresh_ask(&mut self) {
        let Some(from) = self.popups.ask.as_ref().map(|a| a.quote().map(Quote::pane)) else {
            return;
        };
        let targets = self.ask_targets(from);
        if let Some(prompt) = self.popups.ask.as_mut() {
            prompt.refresh(targets);
        }
    }

    fn close_ask(&mut self) {
        self.popups.ask = None;
        self.focus.set_overlay(self.any_overlay());
    }

    /// Every pane running an agent, in tab order, marked when it works in
    /// the checkout of `from` (the pane the quote came from).
    fn ask_targets(&self, from: Option<PaneId>) -> Vec<AskTarget> {
        let checkout = |pane| self.state.place_of(pane).and_then(|p| p.worktree.as_ref());
        let here = from.and_then(checkout);
        self.state
            .all_panes()
            .into_iter()
            .filter(|(_, pane, _)| self.state.is_agent(*pane))
            .map(|(tab, pane, name)| AskTarget {
                pane,
                tab,
                name: name.to_owned(),
                state: self
                    .state
                    .agent_status(pane)
                    .map(|s| s.state)
                    .unwrap_or_default(),
                same_checkout: here.is_some() && checkout(pane) == here,
            })
            .collect()
    }
}
