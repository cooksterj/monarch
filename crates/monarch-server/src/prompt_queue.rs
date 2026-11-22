//! Prompts waiting for an agent to be ready for them: asked while it was
//! working or waiting on the user, typed in once it is at its prompt.
//! Pure data; [`crate::PromptSender`] does the typing.

use std::collections::{BTreeMap, VecDeque};

use monarch_core::PaneId;

/// Each pane's waiting prompts, oldest first, and those now due.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptQueue {
    waiting: BTreeMap<PaneId, VecDeque<String>>,
    due: Vec<(PaneId, String)>,
}

impl PromptQueue {
    /// Queues `text` for `pane`, due at once when the agent is `ready`
    /// and nothing is ahead of it. Returns how many wait before it.
    pub fn push(&mut self, pane: PaneId, text: String, ready: bool) -> usize {
        let ahead = self.waiting(pane);
        if ready && ahead == 0 {
            self.due.push((pane, text));
        } else {
            self.waiting.entry(pane).or_default().push_back(text);
        }
        ahead
    }

    /// `pane`'s agent is at its prompt: its oldest waiting prompt is due.
    /// One at a time, as sending it starts a turn.
    pub fn release(&mut self, pane: PaneId) {
        let Some(queue) = self.waiting.get_mut(&pane) else {
            return;
        };
        if let Some(text) = queue.pop_front() {
            self.due.push((pane, text));
        }
        if queue.is_empty() {
            self.waiting.remove(&pane);
        }
    }

    /// The prompts due, to type in now.
    pub fn take_due(&mut self) -> Vec<(PaneId, String)> {
        std::mem::take(&mut self.due)
    }

    /// How many prompts wait for `pane`.
    #[must_use]
    pub fn waiting(&self, pane: PaneId) -> usize {
        self.waiting.get(&pane).map_or(0, VecDeque::len)
    }

    /// Drops everything for a pane that is gone.
    pub fn forget(&mut self, pane: PaneId) {
        self.waiting.remove(&pane);
        self.due.retain(|(p, _)| *p != pane);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: PaneId = PaneId(3);

    #[test]
    fn a_ready_agent_gets_it_at_once_a_busy_one_when_it_is_ready() {
        let mut q = PromptQueue::default();
        assert_eq!(q.push(P, "now".into(), true), 0);
        assert_eq!(q.take_due(), [(P, "now".to_owned())]);
        assert_eq!(q.push(P, "first".into(), false), 0);
        assert_eq!(q.push(P, "second".into(), true), 1, "behind the first");
        assert!(q.take_due().is_empty(), "nothing due while it works");
        q.release(P);
        assert_eq!(q.take_due(), [(P, "first".to_owned())], "one per turn");
        assert_eq!(q.waiting(P), 1);
        q.release(P);
        assert_eq!(q.take_due(), [(P, "second".to_owned())]);
        q.release(P);
        assert!(q.take_due().is_empty() && q.waiting(P) == 0);
    }

    #[test]
    fn a_pane_that_goes_away_loses_its_prompts() {
        let mut q = PromptQueue::default();
        q.push(P, "a".into(), false);
        q.push(P, "b".into(), false);
        q.release(P);
        q.forget(P);
        assert!(q.take_due().is_empty());
        assert_eq!(q.waiting(P), 0);
        q.release(PaneId(9));
        assert!(q.take_due().is_empty(), "nothing queued there");
    }
}
