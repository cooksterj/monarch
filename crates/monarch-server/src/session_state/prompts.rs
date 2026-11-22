//! Prompts asked of an agent that wasn't ready: queued per pane, due once
//! it is at its prompt.

use monarch_core::PaneId;

use super::SessionState;

impl SessionState {
    /// Queues `text` for `pane`'s agent, due at once if it is at its
    /// prompt; returns how many wait ahead of it, or `None` for no such
    /// pane.
    pub fn queue_prompt(&mut self, pane: PaneId, text: String) -> Option<usize> {
        if !self.panes.contains_key(&pane) {
            return None;
        }
        let ready = self.agents.status(pane).state.takes_prompt();
        Some(self.prompts.push(pane, text, ready))
    }

    /// The prompts due now, to type in.
    pub fn take_due_prompts(&mut self) -> Vec<(PaneId, String)> {
        self.prompts.take_due()
    }
}
