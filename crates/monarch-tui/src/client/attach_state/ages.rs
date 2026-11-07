//! The age shown beside an agent: a stopwatch of its turn's working
//! time, and how long it has been in its state for the inbox.

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

    /// The age to show for a pane's agent at `now`: the turn's working
    /// time, running while Working, held while it waits on the user and
    /// once the turn has ended; `None` with no turn to show.
    pub(super) fn age(&self, sent: &PaneSummary, now: UnixMillis) -> Option<Duration> {
        if self.agent(sent).state == AgentState::Unknown {
            return None;
        }
        self.timing(sent)?.worked_at(now)
    }

    /// How long a pane's agent has been in its state at `now`, if known.
    pub(super) fn waited(&self, sent: &PaneSummary, now: UnixMillis) -> Option<Duration> {
        Some(self.timing(sent)?.since.age_at(now))
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
            self.agent(p).state == AgentState::Working
                && self.timing(p).is_some_and(|t| t.working_since.is_some())
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
    fn the_stopwatch_runs_while_working_and_holds_otherwise() {
        let mut state = state();
        assert_eq!(age(&state), None, "a shell");
        assert!(!state.any_running());
        let running = AgentTiming {
            since: UnixMillis(90_000),
            working_since: Some(UnixMillis(80_000)),
            worked: Some(Duration::from_secs(5)),
        };
        tell(&mut state, AgentState::Working, Some(running));
        assert_eq!(age(&state), Some(25), "worked before plus the span so far");
        assert!(state.any_running());
        let paused = AgentTiming {
            since: UnixMillis(95_000),
            working_since: None,
            worked: Some(Duration::from_secs(25)),
        };
        for held in [
            AgentState::Blocked,
            AgentState::Done,
            AgentState::Finished,
            AgentState::Idle,
            AgentState::Error,
        ] {
            tell(&mut state, held, Some(paused));
            assert_eq!(age(&state), Some(25), "{held:?} holds the stopwatch");
            assert!(!state.any_running(), "{held:?} stands still");
        }
        let layout = state.layout().expect("layout");
        let pane = &layout.tabs[0].panes[0];
        assert_eq!(
            state.waited(pane, NOW),
            Some(Duration::from_secs(5)),
            "but the wait keeps counting"
        );
        let fresh = AgentTiming::begun(UnixMillis(99_000), false);
        tell(&mut state, AgentState::Idle, Some(fresh));
        assert_eq!(age(&state), None, "no turn behind it");
        tell(&mut state, AgentState::Working, None);
        assert_eq!(age(&state), None, "no timing at all");
        assert!(!state.any_running());
    }
}
