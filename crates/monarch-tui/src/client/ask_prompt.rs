//! The ask composer: a prompt for an agent, the agent it goes to, and
//! what it quotes (a selection or a review hunk, or nothing).

use monarch_core::{Action, AgentKind, AgentState};

use super::{AskStep, AskTarget, Picker, Quote};
use crate::{OverlayKey, PickerItem, PickerStep};

const KEYS: &str = " enter send · tab agent · esc cancel ";
/// Below the agents running: starting new ones, asked the prompt first.
const START_DETAIL: &str = "start beside, asked this";

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
        let picker = Picker::prompt(title, items(&targets)).with_text(KEYS, "no agent to ask");
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
        self.picker.refresh(items(&targets));
        self.targets = targets;
    }

    /// One key: type the prompt, Tab or the arrows choose the agent (or
    /// new ones), Enter sends to it (queued until it is at its prompt, if
    /// it is busy or waiting on you) or starts the new ones with it, Esc
    /// closes.
    pub fn key(&mut self, key: OverlayKey) -> AskStep {
        let chosen = match self.picker.key(key) {
            PickerStep::Run(action) => action,
            PickerStep::Close => return AskStep::Close,
            _ => return AskStep::Open,
        };
        let ask = self.picker.query().trim();
        if ask.is_empty() {
            return AskStep::Refuse("write what to ask first".into());
        }
        let text = match &self.quote {
            Some(quote) => quote.message(ask),
            None => ask.to_owned(),
        };
        match chosen {
            Action::FocusPane(pane) => match self.targets.iter().find(|t| t.pane == pane) {
                Some(target) if target.ready() => AskStep::Send { pane, text },
                Some(target) if target.state == AgentState::Exited => {
                    AskStep::Refuse(target.not_ready().unwrap_or_default())
                }
                Some(_) => AskStep::Queue { pane, text },
                None => AskStep::Open,
            },
            Action::LaunchAgent => self
                .picker
                .selected_index()
                .and_then(|i| starts().into_iter().nth(i.checked_sub(self.targets.len())?))
                .map_or(AskStep::Open, |kinds| AskStep::Start { kinds, text }),
            _ => AskStep::Open,
        }
    }
}

/// The rows: the agents running, then each new one to start, then all of
/// them at once.
fn items(targets: &[AskTarget]) -> Vec<PickerItem> {
    let start = starts().into_iter().map(|kinds| {
        let names: Vec<&str> = kinds.iter().map(|k| k.display_name()).collect();
        PickerItem {
            label: format!("+ new {}", names.join(" and ")),
            detail: START_DETAIL.to_owned(),
            action: Action::LaunchAgent,
        }
    });
    targets.iter().map(AskTarget::item).chain(start).collect()
}

/// What the start rows start, in order: each agent alone, then every one.
fn starts() -> Vec<Vec<AgentKind>> {
    let mut starts: Vec<Vec<AgentKind>> = AgentKind::ALL.iter().map(|k| vec![*k]).collect();
    if AgentKind::ALL.len() > 1 {
        starts.push(AgentKind::ALL.to_vec());
    }
    starts
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
    fn busy_or_blocked_agents_queue_exited_and_blank_asks_are_refused() {
        let targets = vec![
            target(1, AgentState::Blocked),
            target(2, AgentState::Idle),
            target(3, AgentState::Exited),
        ];
        let mut prompt = AskPrompt::new(None, targets);
        assert_eq!(prompt.picker().title(), "ask an agent");
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            AskStep::Refuse("write what to ask first".into())
        );
        typed(&mut prompt, "go");
        prompt.key(OverlayKey::Tab);
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            AskStep::Queue {
                pane: PaneId(1),
                text: "go".into()
            },
            "waiting on you: sent once you have answered"
        );
        prompt.key(OverlayKey::Tab);
        let AskStep::Refuse(why) = prompt.key(OverlayKey::Enter) else {
            panic!("an exited agent takes nothing");
        };
        assert!(why.contains("has exited"), "{why}");
        prompt.refresh(vec![
            target(1, AgentState::Done),
            target(2, AgentState::Idle),
            target(3, AgentState::Exited),
        ]);
        assert_eq!(prompt.key(OverlayKey::Esc), AskStep::Close);
    }

    #[test]
    fn with_no_agents_enter_starts_one_asked_the_prompt() {
        let mut prompt = AskPrompt::new(quoted(), Vec::new());
        typed(&mut prompt, "why?");
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            AskStep::Start {
                kinds: vec![AgentKind::Claude],
                text: "why?\n\nSelected in pane 9:\n```\npanic!()\n```".into()
            }
        );
    }

    #[test]
    fn new_agents_are_offered_after_the_running_ones_and_all_at_once_last() {
        let mut prompt = AskPrompt::new(None, vec![target(1, AgentState::Idle)]);
        let labels: Vec<&str> = prompt
            .picker()
            .rows()
            .map(|(i, _)| i.label.as_str())
            .collect();
        assert!(labels[0].ends_with("codex"), "{labels:?}");
        assert_eq!(
            labels[1..],
            [
                "+ new Claude Code",
                "+ new Codex",
                "+ new Claude Code and Codex"
            ]
        );
        typed(&mut prompt, "go");
        for _ in 0..3 {
            prompt.key(OverlayKey::Tab);
        }
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            AskStep::Start {
                kinds: AgentKind::ALL.to_vec(),
                text: "go".into()
            }
        );
    }
}
