//! An agent the ask composer can send to: its pane, how it is listed and
//! whether it can take a prompt now.

use monarch_core::{Action, AgentState, PaneId};

use crate::PickerItem;

/// One agent offered by the composer. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskTarget {
    /// The agent's pane.
    pub pane: PaneId,
    /// Its tab (from 0), as the sidebar counts.
    pub tab: usize,
    /// The pane's name.
    pub name: String,
    /// What its agent is doing.
    pub state: AgentState,
    /// It works in the checkout the quote came from.
    pub same_checkout: bool,
}

impl AskTarget {
    /// Whether text sent now becomes its next turn.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.state.takes_prompt()
    }

    /// Why it can't take the prompt now, if it can't.
    #[must_use]
    pub fn not_ready(&self) -> Option<String> {
        let who = format!("{} in pane {}", self.name, self.pane.0);
        match self.state {
            _ if self.ready() => None,
            AgentState::Blocked => Some(format!("{who} is waiting on you: answer it first")),
            AgentState::Working => Some(format!("{who} is working: ask when it is done")),
            AgentState::Exited => Some(format!("{who} has exited")),
            _ => Some(format!("{who} hasn't reported a prompt yet")),
        }
    }

    /// Ready agents first, then those in the quote's checkout, then tab
    /// order (the sort is stable).
    pub fn rank(targets: &mut [Self]) {
        targets.sort_by_key(|t| (!t.ready(), !t.same_checkout));
    }

    /// Its row: glyph, tab and name; at the right its state, and `here`
    /// for the quote's checkout.
    #[must_use]
    pub fn item(&self) -> PickerItem {
        let state = self.state.label().to_lowercase();
        PickerItem {
            label: format!("{} {} {}", self.state.glyph(), self.tab + 1, self.name),
            detail: if self.same_checkout {
                format!("{state} · here")
            } else {
                state
            },
            action: Action::FocusPane(self.pane),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(pane: u32, state: AgentState, same_checkout: bool) -> AskTarget {
        AskTarget {
            pane: PaneId(pane),
            tab: 0,
            name: "claude".into(),
            state,
            same_checkout,
        }
    }

    #[test]
    fn ready_agents_here_come_first() {
        let mut targets = vec![
            target(1, AgentState::Working, true),
            target(2, AgentState::Idle, false),
            target(3, AgentState::Done, true),
            target(4, AgentState::Blocked, false),
        ];
        AskTarget::rank(&mut targets);
        let order: Vec<u32> = targets.iter().map(|t| t.pane.0).collect();
        assert_eq!(order, [3, 2, 1, 4]);
    }

    #[test]
    fn rows_and_reasons_follow_the_state() {
        let idle = target(2, AgentState::Idle, true);
        assert_eq!(idle.not_ready(), None);
        assert_eq!(idle.item().label, "○ 1 claude");
        assert_eq!(idle.item().detail, "idle · here");
        assert_eq!(idle.item().action, Action::FocusPane(PaneId(2)));
        let blocked = target(4, AgentState::Blocked, false);
        assert_eq!(blocked.item().detail, "blocked");
        assert_eq!(
            blocked.not_ready().as_deref(),
            Some("claude in pane 4 is waiting on you: answer it first")
        );
        let starting = target(5, AgentState::Unknown, false);
        assert!(starting.not_ready().is_some_and(|r| r.contains("yet")));
    }
}
