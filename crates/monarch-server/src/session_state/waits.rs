//! Telling clients and waiters about agent status changes, and answering
//! `agent.wait`.

use monarch_core::PaneId;
use monarch_proto::{AgentStatus, AgentWait, AgentWaitReply, ServerMsg};

use super::SessionState;

impl SessionState {
    /// `pane`'s agent status changed to `status`: remember the state for
    /// waiters and tell every client.
    pub(super) fn agent_changed(&mut self, pane: PaneId, status: AgentStatus) {
        self.agents.record(pane, status.state);
        self.broadcast(&ServerMsg::Agent { pane, status });
    }

    /// Whether the pane in `wait` is in, or since `wait.since` entered, a
    /// state it waits for; `None` when there is no such pane.
    #[must_use]
    pub fn agent_wait(&self, wait: &AgentWait) -> Option<AgentWaitReply> {
        if !self.panes.contains_key(&wait.pane) {
            return None;
        }
        let status = self.agents.status(wait.pane);
        let now = wait.until.contains(&status.state).then_some(status.state);
        let since = wait
            .since
            .and_then(|mark| self.agents.entered_since(wait.pane, &wait.until, mark));
        Some(AgentWaitReply {
            reached: now.or(since),
            status,
            mark: self.agents.changes(),
        })
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentState;

    use super::*;
    use crate::test_support::FakePane;

    fn wait(pane: PaneId, since: Option<u64>) -> AgentWait {
        AgentWait {
            pane,
            until: vec![AgentState::Blocked],
            since,
        }
    }

    #[test]
    fn waits_see_states_that_came_and_went() {
        let mut s = SessionState::default();
        let id = s.allocate_pane_id();
        s.insert_pane(id, FakePane::shell());
        s.set_command(id, Some("gemini".into()));
        s.screen_seen(id, AgentState::Working);
        let first = s.agent_wait(&wait(id, None)).expect("pane");
        assert_eq!(
            (first.reached, first.status.state),
            (None, AgentState::Working)
        );
        s.screen_seen(id, AgentState::Blocked);
        s.screen_seen(id, AgentState::Working);
        let later = s.agent_wait(&wait(id, Some(first.mark))).expect("pane");
        assert_eq!(
            later.reached,
            Some(AgentState::Blocked),
            "missed between looks"
        );
        assert_eq!(s.agent_wait(&wait(id, None)).map(|r| r.reached), Some(None));
        s.screen_seen(id, AgentState::Blocked);
        let now = s.agent_wait(&wait(id, None)).expect("pane");
        assert_eq!(now.reached, Some(AgentState::Blocked), "already there");
        assert_eq!(s.agent_wait(&wait(PaneId(9), None)), None);
    }
}
