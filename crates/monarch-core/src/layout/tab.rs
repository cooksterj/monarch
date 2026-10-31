//! One tab: its split tree, which pane has focus, zoom, and the names
//! the user gave it and its panes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Direction, Rect, SplitAxis, SplitTree};
use crate::{PaneId, given_name};

/// A tab of panes. The focused pane is always in the tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    /// The arrangement of its panes.
    pub tree: SplitTree,
    /// The pane receiving input.
    pub focus: PaneId,
    /// Whether the focused pane fills the tab (tmux `z`).
    pub zoomed: bool,
    /// The name the user gave it; `None` names it after its focused pane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Names the user gave panes in it; the others are named for their
    /// program.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pane_names: BTreeMap<PaneId, String>,
}

impl Tab {
    /// A tab holding just `pane`.
    #[must_use]
    pub fn new(pane: PaneId) -> Self {
        Self {
            tree: SplitTree::new(pane),
            focus: pane,
            zoomed: false,
            name: None,
            pane_names: BTreeMap::new(),
        }
    }

    /// Names the tab `name`, tidied by [`given_name`]; a blank name gives
    /// the automatic one back. Returns whether the name changed.
    pub fn set_name(&mut self, name: &str) -> bool {
        let name = given_name(name);
        let changed = self.name != name;
        self.name = name;
        changed
    }

    /// Names `pane` like [`set_name`](Self::set_name) does the tab.
    /// Returns whether its name changed (never for a pane not in it).
    pub fn set_pane_name(&mut self, pane: PaneId, name: &str) -> bool {
        if !self.contains(pane) {
            return false;
        }
        match given_name(name) {
            Some(name) => self.pane_names.insert(pane, name.clone()) != Some(name),
            None => self.pane_names.remove(&pane).is_some(),
        }
    }

    /// Whether `pane` is in this tab.
    #[must_use]
    pub fn contains(&self, pane: PaneId) -> bool {
        self.tree.contains(pane)
    }

    /// The panes shown and where: just the focused pane when zoomed.
    #[must_use]
    pub fn visible(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        if self.zoomed {
            vec![(self.focus, area)]
        } else {
            self.tree.layout(area)
        }
    }

    /// Splits the focused pane; `new` takes the second half and focus.
    pub fn split(&mut self, new: PaneId, axis: SplitAxis) {
        if self.tree.split(self.focus, new, axis) {
            self.focus = new;
            self.zoomed = false;
        }
    }

    /// Splits the focused pane and zooms `new` over it, so removing `new`
    /// later leaves the tab as it was, focus included.
    pub fn split_zoomed(&mut self, new: PaneId) {
        self.split(new, SplitAxis::Columns);
        self.zoomed = self.focus == new;
    }

    /// Focuses `pane` if it is in this tab; leaves zoom when the zoomed
    /// pane loses focus. Returns whether focus is on `pane`.
    pub fn focus(&mut self, pane: PaneId) -> bool {
        if !self.contains(pane) {
            return false;
        }
        if pane != self.focus {
            self.focus = pane;
            self.zoomed = false;
        }
        true
    }

    /// Moves focus to the neighbour in `dir`. Returns whether it moved.
    pub fn focus_towards(&mut self, area: Rect, dir: Direction) -> bool {
        self.tree
            .neighbour(area, self.focus, dir)
            .is_some_and(|next| self.focus(next))
    }

    /// Moves focus to the next (or previous) pane in reading order.
    pub fn focus_cycle(&mut self, forward: bool) -> bool {
        let panes = self.tree.panes();
        let Some(at) = panes.iter().position(|p| *p == self.focus) else {
            return false;
        };
        let len = panes.len();
        let next = if forward {
            (at + 1) % len
        } else {
            (at + len - 1) % len
        };
        next != at && self.focus(panes[next])
    }

    /// Swaps the focused pane with its neighbour in `dir`; focus moves
    /// with the pane. Returns whether anything moved.
    pub fn swap_towards(&mut self, area: Rect, dir: Direction) -> bool {
        self.tree
            .neighbour(area, self.focus, dir)
            .is_some_and(|other| self.tree.swap(self.focus, other))
    }

    /// Swaps the focused pane with the next (or previous) one in reading
    /// order; focus moves with the pane.
    pub fn swap_cycle(&mut self, forward: bool) -> bool {
        let panes = self.tree.panes();
        let Some(at) = panes.iter().position(|p| *p == self.focus) else {
            return false;
        };
        let len = panes.len();
        let other = if forward {
            (at + 1) % len
        } else {
            (at + len - 1) % len
        };
        other != at && self.tree.swap(self.focus, panes[other])
    }

    /// Renames every pane, the focused one too; see [`SplitTree::rename`].
    pub fn rename(&mut self, rename: &dyn Fn(PaneId) -> PaneId) {
        self.tree.rename(rename);
        self.focus = rename(self.focus);
        self.pane_names = std::mem::take(&mut self.pane_names)
            .into_iter()
            .map(|(pane, name)| (rename(pane), name))
            .collect();
    }

    /// Removes `pane`. When it had focus, focus goes to the pane that took
    /// its space. Returns `true` when the tab is now empty.
    pub fn remove(&mut self, area: Rect, pane: PaneId) -> bool {
        let old = self.tree.layout(area).into_iter().find(|(p, _)| *p == pane);
        if !self.tree.remove(pane) {
            return false;
        }
        self.pane_names.remove(&pane);
        if self.tree.is_empty() {
            return true;
        }
        if pane == self.focus {
            self.zoomed = false;
            let heir = old.and_then(|(_, r)| self.tree.pane_at(area, r.x, r.y));
            self.focus = heir.unwrap_or_else(|| self.tree.panes()[0]);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);
    const C: PaneId = PaneId(3);
    const AREA: Rect = Rect::new(0, 0, 81, 24);

    #[test]
    fn a_zoomed_split_gives_the_layout_back_when_it_goes() {
        let mut tab = Tab::new(A);
        tab.split(B, SplitAxis::Rows);
        tab.focus(A);
        tab.split_zoomed(C);
        assert_eq!((tab.focus, tab.zoomed), (C, true));
        assert_eq!(tab.visible(AREA), vec![(C, AREA)]);
        assert!(!tab.remove(AREA, C));
        assert_eq!((tab.focus, tab.zoomed), (A, false));
        assert_eq!(tab.visible(AREA).len(), 2);
    }

    #[test]
    fn splitting_focuses_the_new_pane_and_unzooms() {
        let mut tab = Tab::new(A);
        tab.zoomed = true;
        tab.split(B, SplitAxis::Columns);
        assert_eq!((tab.focus, tab.zoomed), (B, false));
        assert_eq!(tab.visible(AREA).len(), 2);
    }

    #[test]
    fn zoom_shows_only_the_focused_pane() {
        let mut tab = Tab::new(A);
        tab.split(B, SplitAxis::Rows);
        tab.zoomed = true;
        assert_eq!(tab.visible(AREA), [(B, AREA)]);
        assert!(tab.focus(A));
        assert!(!tab.zoomed, "focusing another pane leaves zoom");
    }

    #[test]
    fn focus_moves_by_direction_and_cycles() {
        let mut tab = Tab::new(A);
        tab.split(B, SplitAxis::Columns);
        assert!(tab.focus_towards(AREA, Direction::Left));
        assert_eq!(tab.focus, A);
        assert!(!tab.focus_towards(AREA, Direction::Left), "edge");
        assert!(tab.focus_cycle(true));
        assert_eq!(tab.focus, B);
        assert!(tab.focus_cycle(true));
        assert_eq!(tab.focus, A, "wraps");
        assert!(!tab.focus(PaneId(9)));
    }

    #[test]
    fn swaps_keep_focus_on_the_moved_pane() {
        let mut tab = Tab::new(A);
        tab.split(B, SplitAxis::Columns); // A | B, B focused
        assert!(tab.swap_towards(AREA, Direction::Left));
        assert_eq!((tab.tree.panes(), tab.focus), (vec![B, A], B));
        assert!(!tab.swap_towards(AREA, Direction::Left), "edge");
        assert!(tab.swap_cycle(true));
        assert_eq!(tab.tree.panes(), [A, B]);
        assert!(!Tab::new(A).swap_cycle(true), "alone");
    }

    #[test]
    fn closing_the_focused_pane_focuses_the_one_that_took_its_space() {
        let mut tab = Tab::new(A);
        tab.split(B, SplitAxis::Columns);
        tab.split(C, SplitAxis::Rows); // B over C, C focused
        assert!(!tab.remove(AREA, C));
        assert_eq!(tab.focus, B);
        assert!(!tab.remove(AREA, A));
        assert!(tab.remove(AREA, B), "now empty");
        assert!(!Tab::new(A).remove(AREA, B), "unknown pane");
    }

    #[test]
    fn names_are_tidied_and_blank_clears() {
        let mut tab = Tab::new(PaneId(1));
        assert!(tab.set_name("  api\nwork "));
        assert_eq!(tab.name.as_deref(), Some("api work"));
        assert!(!tab.set_name("api work"), "unchanged");
        assert!(tab.set_name("   "));
        assert_eq!(tab.name, None, "blank: back to automatic");
        assert!(!tab.set_name(""));
    }

    #[test]
    fn panes_are_named_while_they_are_in_the_tab() {
        let mut tab = Tab::new(A);
        tab.split(B, SplitAxis::Columns);
        assert!(tab.set_pane_name(A, " server "));
        assert!(!tab.set_pane_name(A, "server"), "unchanged");
        assert!(tab.set_pane_name(B, "tests"));
        assert!(!tab.set_pane_name(C, "x"), "not in this tab");
        assert_eq!(tab.pane_names.get(&A).map(String::as_str), Some("server"));
        tab.rename(&|p| PaneId(p.0 + 10));
        assert_eq!(
            tab.pane_names.get(&PaneId(11)).map(String::as_str),
            Some("server")
        );
        tab.remove(AREA, PaneId(12));
        assert_eq!(tab.pane_names.len(), 1, "a closed pane's name goes");
        assert!(tab.set_pane_name(PaneId(11), ""));
        assert!(tab.pane_names.is_empty());
    }

    #[test]
    fn pane_names_survive_the_state_file() {
        let mut tab = Tab::new(A);
        tab.set_pane_name(A, "server");
        let json = serde_json::to_string(&tab).expect("json");
        let back: Tab = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, tab);
    }

    #[test]
    fn unnamed_tabs_keep_their_old_saved_shape() {
        let json = serde_json::to_string(&Tab::new(PaneId(1))).expect("json");
        assert!(!json.contains("name"));
        let back: Tab = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, Tab::new(PaneId(1)));
    }

    #[test]
    fn renaming_moves_focus_and_keeps_zoom() {
        let mut tab = Tab::new(A);
        tab.split(B, SplitAxis::Columns);
        tab.zoomed = true;
        tab.rename(&|p| PaneId(p.0 * 10));
        assert_eq!((tab.focus, tab.zoomed), (PaneId(20), true));
        assert_eq!(tab.tree.panes(), [PaneId(10), PaneId(20)]);
    }
}
