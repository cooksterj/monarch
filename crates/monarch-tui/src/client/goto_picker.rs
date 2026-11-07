//! The go-to picker: every pane in every tab, searched by name, narrowed
//! to blocked, working, idle or done agents with one key.

use monarch_core::{Action, AgentState};

use super::{GoToPane, Picker, PickerItem, PickerStep};
use crate::OverlayKey;

const TITLE: &str = "go to pane";
const KEYS: &str = " ↑↓ move · enter jump · b/w/i/d filter · esc close ";

/// The panes, the state filter and the search over them. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoToPicker {
    picker: Picker,
    panes: Vec<GoToPane>,
    filter: Option<AgentState>,
}

impl GoToPicker {
    /// A picker over `panes`, unfiltered.
    #[must_use]
    pub fn new(panes: Vec<GoToPane>) -> Self {
        let mut picker = Self {
            picker: Picker::new(TITLE, Vec::new()),
            panes,
            filter: None,
        };
        picker.rebuild();
        picker
    }

    /// Replaces the panes (their states move), keeping the search, the
    /// filter and the highlight where they were.
    pub fn refresh(&mut self, panes: Vec<GoToPane>) {
        self.panes = panes;
        self.picker.refresh(self.items());
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// The state the list is narrowed to, if any.
    #[must_use]
    pub fn filter(&self) -> Option<AgentState> {
        self.filter
    }

    /// Applies one key. With nothing typed, `b`, `w`, `i` and `d` narrow
    /// the list to blocked, working, idle or done agents (the same key
    /// again, or Backspace, widens it); every other key searches, moves,
    /// jumps or closes as in any picker.
    pub fn key(&mut self, key: OverlayKey) -> PickerStep {
        if self.picker.query().is_empty() {
            let wanted = match key {
                OverlayKey::Char('b') => Some(AgentState::Blocked),
                OverlayKey::Char('w') => Some(AgentState::Working),
                OverlayKey::Char('i') => Some(AgentState::Idle),
                OverlayKey::Char('d') => Some(AgentState::Done),
                OverlayKey::Backspace if self.filter.is_some() => None,
                _ => return self.picker.key(key),
            };
            self.filter = wanted.filter(|w| Some(*w) != self.filter);
            self.rebuild();
            return PickerStep::Open;
        }
        self.picker.key(key)
    }

    /// Whether `pane` passes the filter: Done also admits Finished, both
    /// drawn `✓`.
    fn admits(&self, pane: &GoToPane) -> bool {
        match self.filter {
            None => true,
            Some(AgentState::Done) => {
                matches!(pane.state, AgentState::Done | AgentState::Finished)
            }
            Some(state) => pane.state == state,
        }
    }

    fn items(&self) -> Vec<PickerItem> {
        self.panes
            .iter()
            .filter(|p| self.admits(p))
            .map(|p| PickerItem {
                label: p.label(),
                detail: p.detail(),
                action: Action::FocusPane(p.pane),
            })
            .collect()
    }

    /// Remakes the picker for the filter, keeping the query.
    fn rebuild(&mut self) {
        let (title, empty) = match self.filter {
            None => (TITLE.to_owned(), "no panes".to_owned()),
            Some(state) => {
                let state = state.label().to_lowercase();
                (format!("{TITLE} · {state}"), format!("no {state} agents"))
            }
        };
        let query = self.picker.query().to_owned();
        self.picker = Picker::new(title, self.items())
            .with_text(KEYS, empty)
            .with_query(query);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;

    use super::*;

    fn pane(n: u32, name: &str, state: AgentState) -> GoToPane {
        GoToPane {
            tab: n as usize - 1,
            pane: PaneId(n),
            name: name.into(),
            state,
            here: n == 1,
        }
    }

    fn panes() -> Vec<GoToPane> {
        vec![
            pane(1, "zsh", AgentState::Unknown),
            pane(2, "claude", AgentState::Blocked),
            pane(3, "codex", AgentState::Working),
            pane(4, "claude", AgentState::Finished),
            pane(5, "build", AgentState::Idle),
        ]
    }

    fn labels(p: &GoToPicker) -> Vec<String> {
        p.picker().rows().map(|(i, _)| i.label.clone()).collect()
    }

    #[test]
    fn filter_keys_narrow_to_a_state_and_toggle_off() {
        let mut p = GoToPicker::new(panes());
        assert_eq!(labels(&p).len(), 5);
        assert_eq!(p.key(OverlayKey::Char('b')), PickerStep::Open);
        assert_eq!(p.filter(), Some(AgentState::Blocked));
        assert_eq!(labels(&p), ["2 claude"]);
        assert_eq!(p.picker().title(), "go to pane · blocked");
        p.key(OverlayKey::Char('d'));
        assert_eq!(labels(&p), ["4 claude"], "done admits finished");
        p.key(OverlayKey::Char('w'));
        assert_eq!(labels(&p), ["3 codex"]);
        p.key(OverlayKey::Char('i'));
        assert_eq!(labels(&p), ["5 build"]);
        assert_eq!(
            p.picker().text().map(|(_, e)| e),
            Some("no idle agents"),
            "the no-rows text names the filter"
        );
        p.key(OverlayKey::Char('i'));
        assert_eq!(p.filter(), None, "the same key again widens");
        assert_eq!(labels(&p).len(), 5);
        p.key(OverlayKey::Char('b'));
        p.key(OverlayKey::Backspace);
        assert_eq!(p.filter(), None, "backspace with nothing typed widens");
        assert_eq!(p.key(OverlayKey::Backspace), PickerStep::Open, "harmless");
    }

    #[test]
    fn typing_searches_within_the_filter_and_filter_keys_then_search() {
        let mut p = GoToPicker::new(panes());
        p.key(OverlayKey::Char('b'));
        for c in "cl".chars() {
            p.key(OverlayKey::Char(c));
        }
        assert_eq!(labels(&p), ["2 claude"]);
        assert_eq!(p.key(OverlayKey::Char('w')), PickerStep::Open);
        assert_eq!(p.filter(), Some(AgentState::Blocked), "w searched");
        assert_eq!(labels(&p), [] as [String; 0]);
        p.key(OverlayKey::Ctrl('u'));
        assert_eq!(labels(&p), ["2 claude"]);
        assert_eq!(
            p.key(OverlayKey::Enter),
            PickerStep::Run(Action::FocusPane(PaneId(2)))
        );
        assert_eq!(p.key(OverlayKey::Esc), PickerStep::Close);
    }

    #[test]
    fn rows_show_here_and_states_and_refresh_keeps_the_filter() {
        let mut p = GoToPicker::new(panes());
        let details: Vec<String> = p.picker().rows().map(|(i, _)| i.detail.clone()).collect();
        assert_eq!(details, ["here", "blocked", "working", "finished", "idle"]);
        p.key(OverlayKey::Char('b'));
        let mut moved = panes();
        moved[1].state = AgentState::Working;
        moved[2].state = AgentState::Blocked;
        p.refresh(moved);
        assert_eq!(labels(&p), ["3 codex"]);
        assert_eq!(p.filter(), Some(AgentState::Blocked));
    }
}
