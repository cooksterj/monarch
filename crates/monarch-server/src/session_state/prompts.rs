//! Prompts asked of an agent that wasn't ready: queued per pane, due once
//! it is at its prompt, listed on the agent's status until then and
//! droppable before they are typed in.

use monarch_core::PaneId;
use monarch_proto::{AgentStatus, ServerMsg};

use super::SessionState;

impl SessionState {
    /// Queues `text` for `pane`'s agent, due at once if it is at its
    /// prompt; returns how many wait ahead of it, or `None` for no such
    /// pane. A prompt left waiting shows on the agent's status.
    pub fn queue_prompt(&mut self, pane: PaneId, text: String) -> Option<usize> {
        if !self.panes.contains_key(&pane) {
            return None;
        }
        let ready = self.agents.status(pane).state.takes_prompt();
        let before = self.prompts.waiting(pane);
        let ahead = self.prompts.push(pane, text, ready);
        if self.prompts.waiting(pane) != before {
            self.queue_changed(pane);
        }
        Some(ahead)
    }

    /// Drops the first prompt waiting for `pane` that reads `text`;
    /// whether there was one (one already typed in is not).
    pub fn drop_queued(&mut self, pane: PaneId, text: &str) -> bool {
        let dropped = self.prompts.drop_queued(pane, text);
        if dropped {
            self.queue_changed(pane);
        }
        dropped
    }

    /// The prompts due now, to type in.
    pub fn take_due_prompts(&mut self) -> Vec<(PaneId, String)> {
        self.prompts.take_due()
    }

    /// `status` as clients see it: with the prompts waiting for `pane`.
    pub(super) fn with_queue(&self, pane: PaneId, mut status: AgentStatus) -> AgentStatus {
        status.queued = self.prompts.queued(pane);
        status
    }

    /// Tells every client what now waits for `pane`'s agent.
    fn queue_changed(&mut self, pane: PaneId) {
        let status = self.with_queue(pane, self.agents.status(pane));
        let timing = self.agents.timing(pane);
        self.broadcast(&ServerMsg::Agent {
            pane,
            status,
            timing,
        });
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentKind, AgentState};
    use monarch_proto::AgentReport;

    use tokio::sync::mpsc::{self, Receiver};

    use super::*;
    use crate::test_support::{FakeClock, FakePane};

    fn pane_state() -> (SessionState, PaneId) {
        let mut state = SessionState::default();
        state.set_clock(FakeClock::at(1_000));
        state.insert_pane(PaneId(1), FakePane::shell());
        (state, PaneId(1))
    }

    /// A client attached to `state`, its greeting read.
    fn attached(state: &mut SessionState) -> Receiver<Vec<ServerMsg>> {
        let (lane, mut rx) = mpsc::channel(64);
        state.add_client(lane);
        while rx.try_recv().is_ok() {}
        rx
    }

    fn working(pane: PaneId) -> AgentReport {
        AgentReport {
            state: Some(AgentState::Working),
            ..AgentReport::new(pane, AgentKind::Claude)
        }
    }

    #[test]
    fn a_waiting_prompt_is_on_the_status_until_it_is_dropped() {
        let (mut state, pane) = pane_state();
        state.report_agent(&working(pane));
        let mut rx = attached(&mut state);
        assert_eq!(state.queue_prompt(pane, "why?".into()), Some(0));
        assert_eq!(state.agent_status(pane).queued, ["why?"]);
        assert_eq!(state.summary(pane).agent.queued, ["why?"]);
        let sent = rx.try_recv().expect("the queue is broadcast");
        assert!(matches!(
            &sent[..],
            [ServerMsg::Agent { status, .. }] if status.queued == ["why?"]
        ));
        assert!(!state.drop_queued(pane, "how?"), "nothing reads that");
        assert!(state.drop_queued(pane, "why?"));
        assert!(state.agent_status(pane).queued.is_empty());
        assert!(rx.try_recv().is_ok(), "the drop is broadcast");
        assert!(state.take_due_prompts().is_empty(), "never typed in");
    }

    #[test]
    fn a_prompt_typed_in_at_once_never_shows_as_queued() {
        let (mut state, pane) = pane_state();
        let idle = AgentReport {
            state: Some(AgentState::Idle),
            ..working(pane)
        };
        state.report_agent(&idle);
        let mut rx = attached(&mut state);
        assert_eq!(state.queue_prompt(pane, "now".into()), Some(0));
        assert!(rx.try_recv().is_err(), "nothing waits, nothing to tell");
        assert!(state.agent_status(pane).queued.is_empty());
        assert_eq!(state.take_due_prompts(), [(pane, "now".to_owned())]);
    }

    #[test]
    fn the_status_after_its_turn_lists_what_still_waits() {
        let (mut state, pane) = pane_state();
        state.report_agent(&working(pane));
        state.queue_prompt(pane, "one".into());
        state.queue_prompt(pane, "two".into());
        let idle = AgentReport {
            state: Some(AgentState::Idle),
            ..working(pane)
        };
        state.report_agent(&idle);
        assert_eq!(state.agent_status(pane).queued, ["two"], "one is due");
        assert_eq!(state.take_due_prompts(), [(pane, "one".to_owned())]);
    }
}
