//! The last thing each agent said, in full, for `agent.read`.
//!
//! A status keeps one line of the last message; an agent waiting on
//! another's answer needs all of it. The message of a turn that ended
//! (Done) or failed (Error) is kept until the next one replaces it, so a
//! reader arriving after the next prompt still gets the last answer.

use monarch_core::{AgentState, PaneId};
use monarch_proto::AgentReport;

use super::StateEngine;

impl StateEngine {
    /// Longest message kept, in bytes (a request line is 64 KiB).
    pub const MAX_SAID: usize = 64 * 1024;

    /// The full text of `pane`'s last answer, if its hooks sent one.
    #[must_use]
    pub fn said(&self, pane: PaneId) -> Option<&str> {
        self.said.get(&pane).map(String::as_str)
    }

    /// Keeps the message of a report that ends a turn.
    pub(super) fn note_said(&mut self, pane: PaneId, report: &AgentReport) {
        if !matches!(report.state, Some(AgentState::Done | AgentState::Error)) {
            return;
        }
        let Some(text) = report.activity.as_deref().map(str::trim) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let mut text = text.to_owned();
        if text.len() > Self::MAX_SAID {
            let cut = (0..=Self::MAX_SAID)
                .rev()
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(0);
            text.truncate(cut);
        }
        self.said.insert(pane, text);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentKind;

    use super::*;

    const P: PaneId = PaneId(1);

    fn report(state: AgentState, text: &str) -> AgentReport {
        AgentReport {
            state: Some(state),
            activity: Some(text.into()),
            ..AgentReport::new(P, AgentKind::Claude)
        }
    }

    #[test]
    fn a_turns_last_message_is_kept_in_full_until_the_next() {
        let mut engine = StateEngine::default();
        assert_eq!(engine.said(P), None);
        engine.report(P, &report(AgentState::Working, "fix the bug"));
        assert_eq!(engine.said(P), None, "a prompt is not an answer");
        engine.report(
            P,
            &report(AgentState::Done, "Fixed it.\n\nTwo tests added.\n"),
        );
        assert_eq!(engine.said(P), Some("Fixed it.\n\nTwo tests added."));
        assert_eq!(
            engine.status(P).activity.as_deref(),
            Some("Fixed it."),
            "the status keeps one line"
        );
        engine.report(P, &report(AgentState::Working, "now the docs"));
        assert_eq!(
            engine.said(P),
            Some("Fixed it.\n\nTwo tests added."),
            "kept through the next prompt"
        );
        engine.report(P, &report(AgentState::Error, "rate limited"));
        assert_eq!(engine.said(P), Some("rate limited"));
        engine.report(P, &report(AgentState::Done, "  "));
        assert_eq!(engine.said(P), Some("rate limited"), "blank says nothing");
        engine.forget(P);
        assert_eq!(engine.said(P), None);
    }

    #[test]
    fn long_messages_are_cut_at_a_character() {
        let mut engine = StateEngine::default();
        let long = "é".repeat(StateEngine::MAX_SAID);
        engine.report(P, &report(AgentState::Done, &long));
        let kept = engine.said(P).expect("kept");
        assert!(kept.len() <= StateEngine::MAX_SAID);
        assert_eq!(kept.len() % 2, 0, "whole characters");
    }
}
