//! What each pane's agent is doing, published to every client.

use monarch_core::PaneId;
use monarch_proto::{AgentReport, AgentStatus, ServerMsg};

use super::SessionState;

impl SessionState {
    /// What `pane`'s agent is doing.
    #[must_use]
    pub fn agent_status(&self, pane: PaneId) -> AgentStatus {
        self.agents.status(pane)
    }

    /// Applies an agent's hook report and tells every client if the
    /// pane's status changed. Returns `false` if there is no such pane.
    pub fn report_agent(&mut self, report: &AgentReport) -> bool {
        if !self.panes.contains_key(&report.pane) {
            return false;
        }
        if let Some(status) = self.agents.report(report) {
            self.broadcast(&ServerMsg::Agent {
                pane: report.pane,
                status,
            });
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentKind, AgentState};
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
        assert!(state.report_agent(&report));
        let status = AgentStatus {
            state: AgentState::Working,
            activity: Some("Bash cargo test".into()),
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
        assert!(state.report_agent(&report), "unchanged is still accepted");
        assert!(rx.try_recv().is_err(), "but sends nothing");
    }

    #[test]
    fn unknown_panes_are_refused() {
        let mut state = SessionState::default();
        let report = AgentReport::new(PaneId(9), AgentKind::Codex);
        assert!(!state.report_agent(&report));
    }
}
