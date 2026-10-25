//! Putting restored panes back in the tabs they were saved in.

use std::collections::{BTreeMap, BTreeSet};

use monarch_core::{PaneId, Tab};

use super::SessionState;

impl SessionState {
    /// Arranges the panes in `saved` tabs (splits, ratios, focus, zoom),
    /// showing tab `active`. `ids` maps each saved pane id to the pane
    /// that came back in its place; a saved pane missing from it is left
    /// out, and a tab left with none is dropped. Panes no saved tab names
    /// keep tabs of their own at the end, so none goes missing.
    pub fn adopt_tabs(&mut self, saved: Vec<Tab>, active: usize, ids: &BTreeMap<PaneId, PaneId>) {
        let before = self.visible_panes();
        let area = self.area();
        let mut tabs: Vec<Tab> = Vec::new();
        let mut placed = BTreeSet::new();
        for mut tab in saved {
            let gone = |p: &PaneId| {
                ids.get(p)
                    .is_none_or(|new| placed.contains(new) || !self.panes.contains_key(new))
            };
            let lost: Vec<_> = tab.tree.panes().into_iter().filter(gone).collect();
            if lost.iter().any(|p| tab.remove(area, *p)) {
                continue;
            }
            tab.rename(&|p| ids.get(&p).copied().unwrap_or(p));
            placed.extend(tab.tree.panes());
            tabs.push(tab);
        }
        let shown = active.min(tabs.len().saturating_sub(1));
        let unplaced = self.panes.keys().filter(|p| !placed.contains(*p));
        tabs.extend(unplaced.map(|p| Tab::new(*p)).collect::<Vec<_>>());
        if tabs.is_empty() {
            return;
        }
        self.tabs = tabs;
        self.active = shown;
        self.arrange(&before);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::SplitAxis;

    use super::*;
    use crate::session_state::DEFAULT_SIZE;
    use crate::test_support::FakePane;

    /// A session of `n` restored panes (ids 1..=n), one tab each.
    fn restored(n: u32) -> SessionState {
        let mut s = SessionState::default();
        for _ in 0..n {
            let id = s.allocate_pane_id();
            s.insert_pane(id, FakePane::boxed(DEFAULT_SIZE));
        }
        s
    }

    /// Saved ids 10, 20, 30 came back as 1, 2, 3.
    fn ids() -> BTreeMap<PaneId, PaneId> {
        (1..=3).map(|n| (PaneId(n * 10), PaneId(n))).collect()
    }

    fn split(a: u32, b: u32, axis: SplitAxis) -> Tab {
        let mut tab = Tab::new(PaneId(a));
        tab.split(PaneId(b), axis);
        tab
    }

    #[test]
    fn saved_tabs_come_back_with_new_ids() {
        let mut s = restored(3);
        let mut first = split(10, 20, SplitAxis::Columns);
        first.focus(PaneId(10));
        s.adopt_tabs(vec![first, Tab::new(PaneId(30))], 1, &ids());
        let tabs = s.tabs();
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].tree.panes(), [PaneId(1), PaneId(2)]);
        assert_eq!(tabs[0].focus, PaneId(1));
        assert_eq!((s.active_tab(), s.focus()), (1, Some(PaneId(3))));
        assert_eq!(s.visible_panes(), [PaneId(3)]);
    }

    #[test]
    fn panes_that_did_not_come_back_are_left_out() {
        let mut s = restored(2);
        let ids: BTreeMap<_, _> = ids().into_iter().take(2).collect();
        let saved = vec![split(10, 30, SplitAxis::Rows), Tab::new(PaneId(30))];
        s.adopt_tabs(saved, 5, &ids);
        let panes: Vec<_> = s.tabs().iter().map(|t| t.tree.panes()).collect();
        assert_eq!(
            panes,
            [vec![PaneId(1)], vec![PaneId(2)]],
            "2 unplaced: own tab"
        );
        assert_eq!(s.active_tab(), 0, "out of range: the last saved tab");
    }

    #[test]
    fn nothing_saved_keeps_one_tab_each() {
        let mut s = restored(2);
        s.adopt_tabs(Vec::new(), 0, &BTreeMap::new());
        assert_eq!(s.tabs().len(), 2);
        let mut empty = SessionState::default();
        empty.adopt_tabs(vec![Tab::new(PaneId(10))], 0, &ids());
        assert!(empty.tabs().is_empty(), "no panes at all");
    }

    #[test]
    fn a_pane_named_twice_is_placed_once() {
        let mut s = restored(2);
        let saved = vec![split(10, 20, SplitAxis::Columns), Tab::new(PaneId(20))];
        s.adopt_tabs(saved, 0, &ids());
        let panes: Vec<_> = s.tabs().iter().map(|t| t.tree.panes()).collect();
        assert_eq!(panes, [vec![PaneId(1), PaneId(2)]]);
    }
}
