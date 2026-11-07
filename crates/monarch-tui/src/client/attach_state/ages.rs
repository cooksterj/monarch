//! The age shown beside an agent: a running count while it works or
//! waits on the user, the frozen length of its last turn once it rests.

use std::time::Duration;

use monarch_core::{AgentState, UnixMillis};
use monarch_proto::{AgentTiming, PaneSummary};

use super::AttachState;

impl AttachState {
    /// A pane's agent's clock facts, from the latest `Agent` or else the
    /// layout.
    pub(super) fn timing(&self, sent: &PaneSummary) -> Option<AgentTiming> {
        self.timing.get(&sent.pane).copied().unwrap_or(sent.timing)
    }

    /// The age to show for a pane's agent at `now`: how long its turn has
    /// run while Working (a prompt in the middle counts), how long it has
    /// waited while Blocked or failed, and how long the last turn took
    /// once it is Done, Finished or Idle; `None` with nothing to show.
    pub(super) fn age(&self, sent: &PaneSummary, now: UnixMillis) -> Option<Duration> {
        let timing = self.timing(sent)?;
        match self.agent(sent).state {
            AgentState::Working => Some(timing.turn_began.unwrap_or(timing.since).age_at(now)),
            AgentState::Blocked | AgentState::Error => Some(timing.since.age_at(now)),
            AgentState::Done | AgentState::Finished | AgentState::Idle => timing.took,
            _ => None,
        }
    }

    /// When a pane's agent entered its state, for ordering by wait.
    pub(super) fn since(&self, sent: &PaneSummary) -> Option<UnixMillis> {
        self.timing(sent).map(|t| t.since)
    }

    /// Whether any pane's age is a running count, so it must be redrawn
    /// as time passes.
    #[must_use]
    pub fn any_running(&self) -> bool {
        let Some(layout) = self.layout.as_ref() else {
            return false;
        };
        let mut panes = layout.tabs.iter().flat_map(|t| t.panes.iter());
        panes.any(|p| {
            let running = matches!(
                self.agent(p).state,
                AgentState::Working | AgentState::Blocked | AgentState::Error
            );
            running && self.timing(p).is_some()
        })
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentKind, PaneId};
    use monarch_proto::{AgentStatus, LayoutMsg, ScreenSize, ServerMsg, TabSummary};

    use super::*;
    use crate::Applied;

    const NOW: UnixMillis = UnixMillis(100_000);

    fn state() -> AttachState {
        let pane = PaneSummary {
            given_name: None,
            pane: PaneId(1),
            title: None,
            command: Some("claude".into()),
            place: None,
            agent: AgentStatus::default(),
            timing: None,
        };
        let layout = LayoutMsg {
            generation: 1,
            size: ScreenSize::new(80, 24),
            focus: Some(PaneId(1)),
            panes: Vec::new(),
            tabs: vec![TabSummary {
                name: None,
                focus: PaneId(1),
                zoomed: false,
                panes: vec![pane],
            }],
            active_tab: 0,
        };
        let mut state = AttachState::new();
        state.apply(ServerMsg::Layout(layout), &mut Applied::default());
        state
    }

    fn tell(state: &mut AttachState, agent: AgentState, timing: Option<AgentTiming>) {
        let status = AgentStatus {
            state: agent,
            activity: None,
            agent: Some(AgentKind::Claude),
        };
        let msg = ServerMsg::Agent {
            pane: PaneId(1),
            status,
            timing,
        };
        state.apply(msg, &mut Applied::default());
    }

    fn age(state: &AttachState) -> Option<u64> {
        let layout = state.layout().expect("layout");
        state
            .age(&layout.tabs[0].panes[0], NOW)
            .map(|a| a.as_secs())
    }

    #[test]
    fn working_counts_the_turn_and_rest_shows_how_long_it_took() {
        let mut state = state();
        assert_eq!(age(&state), None, "a shell");
        assert!(!state.any_running());
        let turn = AgentTiming {
            since: UnixMillis(90_000),
            turn_began: Some(UnixMillis(70_000)),
            took: Some(Duration::from_secs(5)),
        };
        tell(&mut state, AgentState::Working, Some(turn));
        assert_eq!(
            age(&state),
            Some(30),
            "the whole turn, not the last activity"
        );
        assert!(state.any_running());
        tell(&mut state, AgentState::Blocked, Some(turn));
        assert_eq!(age(&state), Some(10), "how long the prompt has waited");
        let rested = AgentTiming {
            since: UnixMillis(95_000),
            turn_began: None,
            took: Some(Duration::from_secs(25)),
        };
        for state_after in [AgentState::Done, AgentState::Finished, AgentState::Idle] {
            tell(&mut state, state_after, Some(rested));
            assert_eq!(
                age(&state),
                Some(25),
                "{state_after:?} shows what the turn took"
            );
            assert!(!state.any_running(), "{state_after:?} stands still");
        }
        tell(&mut state, AgentState::Error, Some(rested));
        assert_eq!(age(&state), Some(5), "how long it has been failed");
        let fresh = AgentTiming::begun(UnixMillis(99_000), false);
        tell(&mut state, AgentState::Idle, Some(fresh));
        assert_eq!(age(&state), None, "no turn behind it");
        tell(&mut state, AgentState::Working, None);
        assert_eq!(age(&state), None, "no timing at all");
        assert!(!state.any_running());
    }
}
