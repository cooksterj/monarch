//! Session commands from clients: splits, focus, zoom, tabs and their
//! names.

use monarch_core::{Action, PaneId, RESIZE_STEP, Tab};
use monarch_proto::{TabRename, TabRenamed};

use super::SessionState;
use crate::Placement;

/// What the caller still has to do after [`SessionState::apply_action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionEffect {
    /// Nothing; the session changed (or the action did not apply).
    Done,
    /// Start a shell pane here (needs a PTY, so it happens outside).
    Spawn(Placement),
}

impl SessionState {
    /// Applies `action` to the session.
    pub fn apply_action(&mut self, action: Action) -> ActionEffect {
        match action {
            Action::ClosePane => self.close_focused(),
            Action::CloseTab => self.close_shown_tab(),
            Action::BreakPane => self.break_focused(),
            Action::FocusPane(id) => {
                self.set_focus(id);
            }
            _ => return self.apply_to_tab(action),
        }
        ActionEffect::Done
    }

    /// Applies `action` to the shown tab.
    fn apply_to_tab(&mut self, action: Action) -> ActionEffect {
        let before = self.visible_panes();
        let area = self.area();
        let tabs = self.tabs.len();
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return ActionEffect::Done;
        };
        let changed = match action {
            Action::Split(axis) => return ActionEffect::Spawn(Placement::Split(axis)),
            Action::NewTab => return ActionEffect::Spawn(Placement::Tab { activate: true }),
            Action::Focus(dir) => tab.focus_towards(area, dir),
            Action::FocusNext => tab.focus_cycle(true),
            Action::FocusPrev => tab.focus_cycle(false),
            Action::Swap(dir) => tab.swap_towards(area, dir),
            Action::SwapNext => tab.swap_cycle(true),
            Action::SwapPrev => tab.swap_cycle(false),
            Action::Resize(dir) => {
                !tab.zoomed && tab.tree.resize(area, tab.focus, dir, RESIZE_STEP)
            }
            Action::ToggleZoom => {
                let many = tab.tree.panes().len() > 1;
                tab.zoomed = many && !tab.zoomed;
                many
            }
            Action::Equalize => {
                tab.tree.equalize();
                true
            }
            Action::NextTab => self.show_tab((self.active + 1) % tabs),
            Action::PrevTab => self.show_tab((self.active + tabs - 1) % tabs),
            Action::SelectTab(n) => self.show_tab(usize::from(n).wrapping_sub(1)),
            _ => false,
        };
        if changed {
            self.arrange(&before);
        }
        ActionEffect::Done
    }

    /// Names the tab holding `pane` (blank: its automatic name again);
    /// clients get the new layout. Returns whether anything changed.
    pub fn rename_tab(&mut self, pane: PaneId, name: &str) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|t| t.contains(pane)) else {
            return false;
        };
        let changed = tab.set_name(name);
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// Renames the tab `request` picks (by number, else by a pane in it,
    /// else the shown one). `None` when there is no such tab.
    pub fn rename_tab_for(&mut self, request: &TabRename) -> Option<TabRenamed> {
        let index = match (request.tab, request.pane) {
            (Some(number), _) => number.checked_sub(1)?,
            (None, Some(pane)) => self.tabs.iter().position(|t| t.contains(pane))?,
            (None, None) => self.active,
        };
        let pane = self.tabs.get(index)?.focus;
        self.rename_tab(pane, &request.name);
        Some(TabRenamed {
            tab: index + 1,
            name: self.tabs.get(index)?.name.clone(),
        })
    }

    /// Names `pane` (blank: its automatic name again); clients get the new
    /// layout. Returns whether anything changed.
    pub fn rename_pane(&mut self, pane: PaneId, name: &str) -> bool {
        let changed = self.tabs.iter_mut().any(|t| t.set_pane_name(pane, name));
        if changed {
            self.generation += 1;
        }
        changed
    }

    /// Ends the focused pane's program; the pane goes when it exits, like
    /// any other.
    fn close_focused(&self) {
        if let Some(entry) = self.focus().and_then(|id| self.panes.get(&id)) {
            entry.io.close();
        }
    }

    /// Ends the program of every pane in the shown tab; the tab goes when
    /// the last one exits.
    fn close_shown_tab(&self) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        for id in tab.tree.panes() {
            if let Some(entry) = self.panes.get(&id) {
                entry.io.close();
            }
        }
    }

    /// Moves the focused pane out of its split into a new tab at the end,
    /// shown and focused, keeping its name. Alone in its tab, it stays.
    fn break_focused(&mut self) {
        let area = self.area();
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let pane = tab.focus;
        if tab.tree.panes().len() < 2 {
            return;
        }
        let before = self.visible_panes();
        let tab = &mut self.tabs[self.active];
        let name = tab.pane_names.remove(&pane);
        tab.remove(area, pane);
        let mut own = Tab::new(pane);
        if let Some(name) = name {
            own.pane_names.insert(pane, name);
        }
        self.tabs.push(own);
        self.active = self.tabs.len() - 1;
        self.arrange(&before);
    }

    /// Shows tab `index`; returns whether that changed anything.
    fn show_tab(&mut self, index: usize) -> bool {
        let changed = index < self.tabs.len() && index != self.active;
        if changed {
            self.active = index;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakePane;

    use std::sync::atomic::Ordering;

    use monarch_core::SplitAxis;

    /// A session with one tab holding `a` beside `b` (focused), and a
    /// second tab holding `c`.
    fn split_and_tab() -> (SessionState, [PaneId; 3]) {
        let mut s = SessionState::default();
        let ids = [0; 3].map(|_| s.allocate_pane_id());
        s.insert_pane_at(ids[0], FakePane::shell(), Placement::Tab { activate: true });
        s.insert_pane_at(
            ids[1],
            FakePane::shell(),
            Placement::Split(SplitAxis::Columns),
        );
        s.insert_pane_at(ids[2], FakePane::shell(), Placement::BACKGROUND);
        (s, ids)
    }

    #[test]
    fn breaking_a_pane_gives_it_a_tab_of_its_own_with_its_name() {
        let (mut s, [a, b, c]) = split_and_tab();
        assert!(s.rename_pane(b, "api"));
        let before = s.layout().generation;
        assert_eq!(s.apply_action(Action::BreakPane), ActionEffect::Done);
        let tabs = s.tabs();
        assert_eq!(tabs.len(), 3);
        assert_eq!(tabs[0].tree.panes(), [a], "the sibling keeps the tab");
        assert_eq!(tabs[0].focus, a);
        assert_eq!(tabs[1].tree.panes(), [c]);
        assert_eq!(tabs[2].tree.panes(), [b], "a new tab at the end");
        assert_eq!(tabs[2].pane_names.get(&b).map(String::as_str), Some("api"));
        assert_eq!(s.active_tab(), 2, "shown");
        assert_eq!(s.focus(), Some(b));
        assert!(s.layout().generation > before, "clients get the layout");
    }

    #[test]
    fn a_pane_alone_in_its_tab_stays_put() {
        let (mut s, [_, _, c]) = split_and_tab();
        s.set_focus(c);
        let before = s.layout();
        s.apply_action(Action::BreakPane);
        let after = s.layout();
        assert_eq!(after.tabs, before.tabs);
        assert_eq!(after.generation, before.generation, "nothing changed");
    }

    #[test]
    fn closing_a_tab_ends_every_pane_in_it_and_no_other() {
        let mut s = SessionState::default();
        let (a_io, a_closed) = FakePane::tracked();
        let (b_io, b_closed) = FakePane::tracked();
        let (c_io, c_closed) = FakePane::tracked();
        let a = s.allocate_pane_id();
        s.insert_pane_at(a, a_io, Placement::Tab { activate: true });
        let b = s.allocate_pane_id();
        s.insert_pane_at(b, b_io, Placement::Split(SplitAxis::Rows));
        let c = s.allocate_pane_id();
        s.insert_pane_at(c, c_io, Placement::BACKGROUND);
        assert_eq!(s.apply_action(Action::CloseTab), ActionEffect::Done);
        assert!(a_closed.load(Ordering::SeqCst) && b_closed.load(Ordering::SeqCst));
        assert!(
            !c_closed.load(Ordering::SeqCst),
            "the other tab is untouched"
        );
        // The panes go as their programs exit, the tab with the last one.
        s.remove_pane(a);
        assert_eq!(s.tabs().len(), 2);
        s.remove_pane(b);
        assert_eq!(s.tabs().len(), 1);
        assert_eq!(s.focus(), Some(c));
    }

    #[test]
    fn renaming_names_the_tab_holding_the_pane() {
        let mut s = SessionState::default();
        let id = s.allocate_pane_id();
        s.insert_pane(id, FakePane::boxed(crate::DEFAULT_SIZE));
        let before = s.layout().generation;
        assert!(s.rename_tab(id, " api "));
        let layout = s.layout();
        assert_eq!(layout.tabs[0].name.as_deref(), Some("api"));
        assert!(layout.generation > before, "clients get the new layout");
        assert!(!s.rename_tab(id, "api"), "unchanged");
        assert!(!s.rename_tab(PaneId(99), "x"), "no such pane");
        assert!(s.rename_tab(id, ""));
        assert_eq!(s.layout().tabs[0].name, None);
    }

    #[test]
    fn the_api_picks_a_tab_by_number_pane_or_the_shown_one() {
        let mut s = SessionState::default();
        let id = s.allocate_pane_id();
        s.insert_pane(id, FakePane::boxed(crate::DEFAULT_SIZE));
        let ask = |tab, pane, name: &str| TabRename {
            tab,
            pane,
            name: name.into(),
        };
        let shown = s.rename_tab_for(&ask(None, None, "web"));
        assert_eq!(
            shown,
            Some(TabRenamed {
                tab: 1,
                name: Some("web".into())
            })
        );
        let by_pane = s.rename_tab_for(&ask(None, Some(id), " "));
        assert_eq!(by_pane, Some(TabRenamed { tab: 1, name: None }), "cleared");
        assert!(s.rename_tab_for(&ask(Some(1), None, "api")).is_some());
        assert_eq!(s.rename_tab_for(&ask(Some(2), None, "x")), None, "no tab 2");
        assert_eq!(
            s.rename_tab_for(&ask(Some(0), None, "x")),
            None,
            "tabs count from 1"
        );
        assert_eq!(s.rename_tab_for(&ask(None, Some(PaneId(9)), "x")), None);
    }

    #[test]
    fn renaming_a_pane_sends_its_name_with_it() {
        let mut s = SessionState::default();
        let id = s.allocate_pane_id();
        s.insert_pane(id, FakePane::boxed(crate::DEFAULT_SIZE));
        let before = s.layout().generation;
        assert!(s.rename_pane(id, "server"));
        let layout = s.layout();
        assert_eq!(
            layout.tabs[0].panes[0].given_name.as_deref(),
            Some("server")
        );
        assert!(layout.generation > before);
        assert!(!s.rename_pane(id, "server"), "unchanged");
        assert!(!s.rename_pane(PaneId(99), "x"), "no such pane");
    }
}
