//! Reading agent panes' screens for detection, and applying what their
//! manifests concluded.

use std::collections::BTreeSet;

use monarch_core::{AgentKind, AgentState, PaneId};

use super::SessionState;
use crate::screen_watcher::PaneScreen;

impl SessionState {
    /// Every pane whose foreground program `wanted` accepts, with its
    /// screen text when there was output since the last read or the pane
    /// is not in `known` (newly watched).
    pub(crate) fn screens(
        &self,
        wanted: impl Fn(&str) -> bool,
        known: &BTreeSet<PaneId>,
    ) -> Vec<PaneScreen> {
        self.panes
            .iter()
            .filter_map(|(id, entry)| {
                let program = entry.command.clone().filter(|c| wanted(c))?;
                let shared = entry.io.shared();
                let fresh = shared.take_unread() || !known.contains(id);
                Some(PaneScreen {
                    pane: *id,
                    program,
                    title: entry.title.clone().unwrap_or_default(),
                    text: fresh.then(|| shared.lock().text()),
                })
            })
            .collect()
    }

    /// `pane`'s screen shows its agent in `state`; tells every client if
    /// its status changed. A turn that ends while nobody looks at the
    /// pane is Done.
    pub fn screen_seen(&mut self, pane: PaneId, state: AgentState) {
        let Some(entry) = self.panes.get(&pane) else {
            return;
        };
        let agent = entry.command.as_deref().and_then(AgentKind::for_program);
        let watched = self.client_count() > 0 && self.focus() == Some(pane);
        if let Some(status) = self.agents.screen(pane, state, agent, watched) {
            self.agent_changed(pane, status);
        }
    }

    /// The agent detected in `pane` is no longer in front.
    pub fn screen_gone(&mut self, pane: PaneId) {
        if let Some(status) = self.agents.screen_gone(pane) {
            self.agent_changed(pane, status);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::SessionState;
    use crate::test_support::FakePane;

    use super::*;

    fn session() -> (SessionState, PaneId) {
        let mut s = SessionState::default();
        let id = s.allocate_pane_id();
        s.insert_pane(id, FakePane::shell());
        s.set_command(id, Some("gemini".into()));
        (s, id)
    }

    #[test]
    fn screens_are_read_when_new_or_changed() {
        let (s, id) = session();
        let wanted = |p: &str| p == "gemini";
        let mut known = BTreeSet::new();
        let first = s.screens(wanted, &known);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].program, "gemini");
        assert!(first[0].text.is_some(), "new panes are read");
        known.insert(id);
        assert_eq!(s.screens(wanted, &known)[0].text, None, "nothing new");
        let shared = s.pane(id).expect("pane").shared();
        shared.lock().feed(b"> ready", &mut Vec::new());
        shared.mark_dirty();
        let text = s.screens(wanted, &known)[0].text.clone();
        assert!(text.is_some_and(|t| t.starts_with("> ready")));
        assert!(s.screens(|p| p == "codex", &known).is_empty());
    }

    #[test]
    fn unwatched_turns_end_done_and_gone_agents_leave_a_shell() {
        let (mut s, id) = session();
        s.screen_seen(id, AgentState::Working);
        s.screen_seen(id, AgentState::Idle);
        assert_eq!(
            s.agent_status(id).state,
            AgentState::Done,
            "no client watching"
        );
        s.screen_gone(id);
        assert_eq!(s.agent_status(id), Default::default());
        s.screen_seen(PaneId(99), AgentState::Working);
        assert_eq!(
            s.agent_status(PaneId(99)),
            Default::default(),
            "no such pane"
        );
    }
}
