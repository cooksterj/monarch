//! The ask composer: a prompt for an agent, the agent it goes to, and
//! what it quotes (a selection or a review hunk, or nothing).

use monarch_core::Action;

use super::{AskStep, AskTarget, Picker, Quote};
use crate::{OverlayKey, PickerStep};

const KEYS: &str = " enter send · tab agent · esc cancel ";

/// The prompt typed so far over the agents it can go to, ranked, and the
/// quote. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskPrompt {
    picker: Picker,
    targets: Vec<AskTarget>,
    quote: Option<Quote>,
}

impl AskPrompt {
    /// A composer quoting `quote` to one of `targets`, best first.
    #[must_use]
    pub fn new(quote: Option<Quote>, mut targets: Vec<AskTarget>) -> Self {
        AskTarget::rank(&mut targets);
        let title = match &quote {
            Some(quote) => format!("ask about {}", quote.summary()),
            None => "ask an agent".to_owned(),
        };
        let items = targets.iter().map(AskTarget::item).collect();
        let picker = Picker::prompt(title, items).with_text(KEYS, "no agent to ask");
        Self {
            picker,
            targets,
            quote,
        }
    }

    /// What it quotes.
    #[must_use]
    pub fn quote(&self) -> Option<&Quote> {
        self.quote.as_ref()
    }

    /// The picker it draws as.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Replaces the agents (their states change while it is open),
    /// keeping the highlight on the same agent.
    pub fn refresh(&mut self, mut targets: Vec<AskTarget>) {
        AskTarget::rank(&mut targets);
        self.picker
            .refresh(targets.iter().map(AskTarget::item).collect());
        self.targets = targets;
    }

    /// One key: type the prompt, Tab or the arrows choose the agent,
    /// Enter sends to it if it can take a prompt now, Esc closes.
    pub fn key(&mut self, key: OverlayKey) -> AskStep {
        match self.picker.key(key) {
            PickerStep::Run(Action::FocusPane(pane)) => {
                let ask = self.picker.query().trim();
                let Some(target) = self.targets.iter().find(|t| t.pane == pane) else {
                    return AskStep::Open;
                };
                if ask.is_empty() {
                    return AskStep::Refuse("write what to ask first".into());
                }
                if let Some(why) = target.not_ready() {
                    return AskStep::Refuse(why);
                }
                let text = match &self.quote {
                    Some(quote) => quote.message(ask),
                    None => ask.to_owned(),
                };
                AskStep::Send { pane, text }
            }
            PickerStep::Close => AskStep::Close,
            _ => AskStep::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentState, PaneId};

    use super::*;

    fn target(pane: u32, state: AgentState) -> AskTarget {
        AskTarget {
            pane: PaneId(pane),
            tab: 0,
            name: "codex".into(),
            state,
            same_checkout: false,
        }
    }

    fn typed(prompt: &mut AskPrompt, text: &str) {
        for c in text.chars() {
            assert_eq!(prompt.key(OverlayKey::Char(c)), AskStep::Open);
        }
    }

    fn quoted() -> Option<Quote> {
        Some(Quote::Screen {
            pane: PaneId(9),
            text: "panic!()".into(),
        })
    }

    #[test]
    fn enter_sends_the_ask_and_quote_to_a_ready_agent() {
        let targets = vec![target(1, AgentState::Working), target(2, AgentState::Idle)];
        let mut prompt = AskPrompt::new(quoted(), targets);
        assert_eq!(prompt.picker().title(), "ask about 1 line from pane 9");
        assert_eq!(prompt.quote().map(Quote::pane), Some(PaneId(9)));
        typed(&mut prompt, "why?");
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            AskStep::Send {
                pane: PaneId(2),
                text: "why?\n\nSelected in pane 9:\n```\npanic!()\n```".into()
            },
            "the idle agent is offered first"
        );
    }

    #[test]
    fn busy_or_blocked_agents_and_blank_asks_are_refused() {
        let targets = vec![target(1, AgentState::Blocked), target(2, AgentState::Idle)];
        let mut prompt = AskPrompt::new(None, targets);
        assert_eq!(prompt.picker().title(), "ask an agent");
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            AskStep::Refuse("write what to ask first".into())
        );
        typed(&mut prompt, "go");
        assert_eq!(prompt.key(OverlayKey::Tab), AskStep::Open);
        let AskStep::Refuse(why) = prompt.key(OverlayKey::Enter) else {
            panic!("a blocked agent takes no prompt");
        };
        assert!(why.contains("waiting on you"), "{why}");
        prompt.refresh(vec![
            target(1, AgentState::Done),
            target(2, AgentState::Idle),
        ]);
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            AskStep::Send {
                pane: PaneId(1),
                text: "go".into()
            },
            "the highlight stayed on pane 1, which is ready now"
        );
        assert_eq!(prompt.key(OverlayKey::Esc), AskStep::Close);
    }

    #[test]
    fn with_no_agents_enter_does_nothing() {
        let mut prompt = AskPrompt::new(None, Vec::new());
        typed(&mut prompt, "x");
        assert_eq!(prompt.key(OverlayKey::Enter), AskStep::Open);
    }
}
