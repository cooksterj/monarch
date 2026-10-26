//! What each pane's agent is doing, published to every client, and which
//! pane a report that names none belongs to.

use std::path::Path;

use monarch_core::{AgentKind, PaneId};
use monarch_detect::OscSignal;
use monarch_proto::{AgentReport, AgentStatus, StatusSource};

use super::SessionState;

impl SessionState {
    /// What `pane`'s agent is doing.
    #[must_use]
    pub fn agent_status(&self, pane: PaneId) -> AgentStatus {
        self.agents.status(pane)
    }

    /// Applies an agent's hook report and tells every client if the
    /// pane's status changed. Returns the pane, or `None` if the report
    /// belongs to no pane in this session.
    pub fn report_agent(&mut self, report: &AgentReport) -> Option<PaneId> {
        let pane = self.locate(report)?;
        if let Some(status) = self.agents.report(pane, report) {
            self.agent_changed(pane, status);
        }
        Some(pane)
    }

    /// Which signal `pane`'s status came from.
    #[must_use]
    pub fn agent_source(&self, pane: PaneId) -> StatusSource {
        self.agents.source(pane)
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
        let agent = command.and_then(AgentKind::for_program);
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
    use monarch_core::{AgentKind, AgentState};
    use monarch_proto::ServerMsg;
    use tokio::sync::mpsc;

    use super::*;
    use crate::test_support::FakePane;

    #[test]
    fn reports_reach_clients_and_the_layout() {
        let mut state = SessionState::default();
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
        };
        let sent = rx.try_recv().expect("a message");
        assert_eq!(
            sent,
            vec![ServerMsg::Agent {
                pane: PaneId(1),
                status: status.clone()
            }]
        );
        assert_eq!(state.summary(PaneId(1)).agent, status);
        assert!(
            state.report_agent(&report).is_some(),
            "unchanged is still accepted"
        );
        assert!(rx.try_recv().is_err(), "but sends nothing");
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
