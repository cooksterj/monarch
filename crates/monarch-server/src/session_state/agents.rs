//! What each pane's agent is doing, published to every client, and which
//! pane a report that names none belongs to.

use std::collections::BTreeMap;
use std::path::Path;

use monarch_core::{AgentKind, AgentState, PaneId};
use monarch_detect::OscSignal;
use monarch_proto::{AgentReport, AgentStatus, AgentTransition, StatusSource};

use super::SessionState;

impl SessionState {
    /// Names the programs that count as agents (configured wrappers
    /// beside the built-in names).
    pub fn set_agent_programs(&mut self, programs: BTreeMap<String, AgentKind>) {
        self.agent_programs = programs;
    }

    /// The agent `program` (a foreground process name) is, if any.
    #[must_use]
    pub fn agent_for_program(&self, program: &str) -> Option<AgentKind> {
        self.agent_programs.get(program).copied()
    }

    /// What `pane`'s agent is doing.
    #[must_use]
    pub fn agent_status(&self, pane: PaneId) -> AgentStatus {
        self.agents.status(pane)
    }

    /// Applies an agent's hook report and tells every client if the
    /// pane's status changed. A turn that ends in the pane someone is
    /// looking at has been seen: Finished, not Done. Returns the pane,
    /// or `None` if the report belongs to no pane in this session.
    pub fn report_agent(&mut self, report: &AgentReport) -> Option<PaneId> {
        let pane = self.locate(report)?;
        if let Some(mut status) = self.agents.report(pane, report) {
            if status.state == AgentState::Done && self.watched(pane) {
                status = self.agents.seen(pane).unwrap_or(status);
            }
            self.agent_changed(pane, status);
        }
        Some(pane)
    }

    /// Whether someone is looking at `pane`: a client is attached and
    /// the pane is focused.
    #[must_use]
    pub fn watched(&self, pane: PaneId) -> bool {
        self.client_count() > 0 && self.focus() == Some(pane)
    }

    /// Which signal `pane`'s status came from.
    #[must_use]
    pub fn agent_source(&self, pane: PaneId) -> StatusSource {
        self.agents.source(pane)
    }

    /// The states `pane`'s agent entered lately, oldest first.
    #[must_use]
    pub fn agent_transitions(&self, pane: PaneId) -> Vec<AgentTransition> {
        self.agents.transitions(pane)
    }

    /// The full text of `pane`'s last answer, if its hooks sent one.
    #[must_use]
    pub fn agent_said(&self, pane: PaneId) -> Option<&str> {
        self.agents.said(pane)
    }

    /// Monarch started `kind` in `pane`; tells every client.
    pub fn launched_agent(&mut self, pane: PaneId, kind: AgentKind) {
        if let Some(status) = self.agents.launched(pane, kind) {
            self.agent_changed(pane, status);
        }
    }

    /// Applies a notification from `pane`'s program (ignored where hooks
    /// report) and tells every client if its status changed.
    pub fn signal_agent(&mut self, pane: PaneId, signal: &OscSignal) {
        let command = self.panes.get(&pane).and_then(|e| e.command.as_deref());
        let agent = command.and_then(|c| self.agent_for_program(c));
        if let Some(status) = self.agents.signal(pane, signal, agent) {
            self.agent_changed(pane, status);
        }
    }

    /// Focus moved to `pane`: the user has seen it, so a Done agent there
    /// is Idle now.
    pub fn saw(&mut self, pane: PaneId) {
        if let Some(status) = self.agents.seen(pane) {
            self.agent_changed(pane, status);
        }
    }

    /// The user typed `bytes` into `pane`: a reply ends a guessed Blocked
    /// or Done.
    pub fn answered(&mut self, pane: PaneId, bytes: &[u8]) {
        if Self::is_reply(bytes) {
            self.replied(pane);
        }
    }

    /// Whether typing `bytes` answers an agent: Enter, or a key that isn't
    /// an escape sequence (arrows and focus reports are not replies).
    #[must_use]
    pub fn is_reply(bytes: &[u8]) -> bool {
        bytes.iter().any(|b| matches!(b, b'\r' | b'\n'))
            || (!bytes.contains(&0x1b) && bytes.iter().any(u8::is_ascii_graphic))
    }

    /// The user replied in `pane`: a guessed Blocked or Done is over.
    pub fn replied(&mut self, pane: PaneId) {
        if let Some(status) = self.agents.answered(pane) {
            self.agent_changed(pane, status);
        }
    }

    /// The pane a report is about: the one it names; else the one its
    /// session was seen in; else a pane whose foreground program is the
    /// agent, working in the report's directory (one not yet holding a
    /// session first).
    fn locate(&self, report: &AgentReport) -> Option<PaneId> {
        if let Some(pane) = report.pane {
            return self.panes.contains_key(&pane).then_some(pane);
        }
        let seen = report
            .session_id
            .as_deref()
            .and_then(|id| self.agents.pane_for(id));
        if let Some(pane) = seen.filter(|p| self.panes.contains_key(p)) {
            return Some(pane);
        }
        let cwd = Path::new(report.cwd.as_deref()?);
        let program = report.agent.program();
        let candidates: Vec<PaneId> = self
            .panes
            .iter()
            .filter(|(_, e)| e.command.as_deref() == Some(program))
            .filter(|(_, e)| e.place.as_ref().is_some_and(|p| Path::new(&p.cwd) == cwd))
            .map(|(id, _)| *id)
            .collect();
        let free = candidates.iter().find(|p| !self.agents.has_session(**p));
        free.or(candidates.first()).copied()
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentKind;
    use monarch_proto::ServerMsg;
    use tokio::sync::mpsc;

    use monarch_core::{AgentState, UnixMillis};
    use monarch_proto::AgentTiming;

    use super::*;
    use crate::test_support::{FakeClock, FakePane};

    #[test]
    fn reports_reach_clients_and_the_layout() {
        let mut state = SessionState::default();
        state.set_clock(FakeClock::at(1_000));
        state.insert_pane(PaneId(1), FakePane::shell());
        let (lane, mut rx) = mpsc::channel(64);
        state.add_client(lane);
        while rx.try_recv().is_ok() {}
        let report = AgentReport {
            state: Some(AgentState::Working),
            activity: Some("Bash cargo test".into()),
            ..AgentReport::new(PaneId(1), AgentKind::Claude)
        };
        assert_eq!(state.report_agent(&report), Some(PaneId(1)));
        let status = AgentStatus {
            state: AgentState::Working,
            activity: Some("Bash cargo test".into()),
            agent: Some(AgentKind::Claude),
            subagents: Vec::new(),
        };
        let sent = rx.try_recv().expect("a message");
        assert_eq!(
            sent,
            vec![ServerMsg::Agent {
                pane: PaneId(1),
                status: status.clone(),
                timing: Some(AgentTiming::begun(UnixMillis(1_000), true)),
            }]
        );
        let summary = state.summary(PaneId(1));
        assert_eq!(summary.agent, status);
        assert_eq!(summary.timing.map(|t| t.since), Some(UnixMillis(1_000)));
        assert!(
            state.report_agent(&report).is_some(),
            "unchanged is still accepted"
        );
        assert!(rx.try_recv().is_err(), "but sends nothing");
    }

    #[test]
    fn a_state_keeps_its_start_until_the_state_changes() {
        let mut state = SessionState::default();
        let clock = FakeClock::at(1_000);
        state.set_clock(clock.clone());
        state.insert_pane(PaneId(1), FakePane::shell());
        let (lane, mut rx) = mpsc::channel(64);
        state.add_client(lane);
        while rx.try_recv().is_ok() {}
        let report = AgentReport {
            state: Some(AgentState::Working),
            ..AgentReport::new(PaneId(1), AgentKind::Claude)
        };
        state.report_agent(&report);
        let timing = state.summary(PaneId(1)).timing;
        assert_eq!(timing.map(|t| t.since), Some(UnixMillis(1_000)));
        clock.set(61_000);
        let busier = AgentReport {
            activity: Some("Edit a.rs".into()),
            ..report.clone()
        };
        state.report_agent(&busier);
        let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok())
            .flatten()
            .collect();
        let stamps: Vec<Option<AgentTiming>> = sent
            .iter()
            .filter_map(|m| match m {
                ServerMsg::Agent { timing, .. } => Some(*timing),
                _ => None,
            })
            .collect();
        assert_eq!(stamps, [timing, timing], "still working since the start");
        let blocked = AgentReport {
            state: Some(AgentState::Blocked),
            ..report.clone()
        };
        state.report_agent(&blocked);
        let waiting = state.summary(PaneId(1)).timing.expect("timing");
        assert_eq!(waiting.since, UnixMillis(61_000));
        assert_eq!(waiting.working_since, None, "paused for the prompt");
        assert_eq!(waiting.worked, Some(std::time::Duration::from_secs(60)));
    }

    /// A Claude hook report putting `pane` in `state`.
    fn claude(pane: PaneId, state: AgentState) -> AgentReport {
        AgentReport {
            state: Some(state),
            activity: Some("Fixed it.".into()),
            ..AgentReport::new(pane, AgentKind::Claude)
        }
    }

    #[test]
    fn a_turn_ending_in_the_watched_pane_is_finished_not_done() {
        let mut state = SessionState::default();
        state.set_clock(FakeClock::at(1_000));
        state.insert_pane(PaneId(1), FakePane::shell());
        state.insert_pane(PaneId(2), FakePane::shell());
        state.report_agent(&claude(PaneId(1), AgentState::Done));
        assert_eq!(
            state.agent_status(PaneId(1)).state,
            AgentState::Done,
            "nobody is attached"
        );
        let (lane, _rx) = mpsc::channel(64);
        state.add_client(lane);
        let focus = state.focus();
        let other = [PaneId(1), PaneId(2)]
            .into_iter()
            .find(|p| Some(*p) != focus)
            .expect("two panes");
        state.report_agent(&claude(other, AgentState::Done));
        assert_eq!(state.agent_status(other).state, AgentState::Done);
        let focused = focus.expect("a focused pane");
        state.report_agent(&claude(focused, AgentState::Working));
        state.report_agent(&claude(focused, AgentState::Done));
        let status = state.agent_status(focused);
        assert_eq!(status.state, AgentState::Finished, "seen as it happened");
        assert_eq!(status.activity.as_deref(), Some("Fixed it."));
        assert!(state.watched(focused));
        assert!(!state.watched(other));
    }

    #[test]
    fn a_configured_wrapper_counts_as_its_agent() {
        let mut state = SessionState::default();
        assert_eq!(state.agent_for_program("claude"), Some(AgentKind::Claude));
        assert_eq!(state.agent_for_program("my-claude"), None);
        let mut programs = BTreeMap::new();
        programs.insert("my-claude".to_owned(), AgentKind::Claude);
        state.set_agent_programs(programs);
        assert_eq!(
            state.agent_for_program("my-claude"),
            Some(AgentKind::Claude)
        );
        assert_eq!(state.agent_for_program("claude"), None, "as given");
    }

    #[test]
    fn unknown_panes_are_refused() {
        let mut state = SessionState::default();
        let report = AgentReport::new(PaneId(9), AgentKind::Codex);
        assert_eq!(state.report_agent(&report), None);
    }

    fn codex_in(state: &mut SessionState, pane: u32, cwd: &str) {
        state.insert_pane(PaneId(pane), FakePane::shell());
        state.set_command(PaneId(pane), Some("codex".into()));
        let place = monarch_proto::PanePlace::outside(cwd);
        state.set_place(PaneId(pane), Some(place));
    }

    fn unplaced(session: &str, cwd: &str) -> AgentReport {
        AgentReport {
            state: Some(AgentState::Working),
            session_id: Some(session.into()),
            cwd: Some(cwd.into()),
            ..AgentReport::unplaced(AgentKind::Codex)
        }
    }

    #[test]
    fn unplaced_reports_find_the_agent_by_directory_then_session() {
        let mut state = SessionState::default();
        state.insert_pane(PaneId(1), FakePane::shell());
        codex_in(&mut state, 2, "/w/api");
        codex_in(&mut state, 3, "/w/api");
        codex_in(&mut state, 4, "/w/web");
        assert_eq!(
            state.report_agent(&unplaced("a", "/w/api")),
            Some(PaneId(2))
        );
        assert_eq!(
            state.report_agent(&unplaced("b", "/w/api")),
            Some(PaneId(3)),
            "a free pane first"
        );
        assert_eq!(
            state.report_agent(&unplaced("c", "/w/web")),
            Some(PaneId(4))
        );
        let moved = AgentReport {
            cwd: Some("/elsewhere".into()),
            ..unplaced("b", "")
        };
        assert_eq!(
            state.report_agent(&moved),
            Some(PaneId(3)),
            "a known session stays put"
        );
        assert_eq!(state.report_agent(&unplaced("d", "/nowhere")), None);
        let mut claude = unplaced("e", "/w/api");
        claude.agent = AgentKind::Claude;
        assert_eq!(
            state.report_agent(&claude),
            None,
            "only panes running that agent"
        );
    }

    #[test]
    fn notifications_reach_clients_until_answered() {
        let mut state = SessionState::default();
        codex_in(&mut state, 1, "/w");
        let (lane, mut rx) = mpsc::channel(64);
        state.add_client(lane);
        while rx.try_recv().is_ok() {}
        let signal = OscSignal::parse(9, "Approval requested: rm").expect("signal");
        state.signal_agent(PaneId(1), &signal);
        let status = state.agent_status(PaneId(1));
        assert_eq!(
            (status.state, status.agent),
            (AgentState::Blocked, Some(AgentKind::Codex))
        );
        assert!(rx.try_recv().is_ok(), "clients told");
        state.answered(PaneId(1), b"\x1b[A");
        state.answered(PaneId(1), b"\x1b[I");
        assert_eq!(
            state.agent_status(PaneId(1)).state,
            AgentState::Blocked,
            "keys that aren't replies"
        );
        state.answered(PaneId(1), b"y");
        assert_eq!(state.agent_status(PaneId(1)).state, AgentState::Idle);
    }
}
