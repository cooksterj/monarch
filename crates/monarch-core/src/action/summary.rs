//! What each action does in a few words, and which area it belongs to:
//! the help page's text.

use super::Action;
use crate::{ActionGroup, SplitAxis};

impl Action {
    /// What it does, in one short line.
    #[must_use]
    pub fn summary(self) -> &'static str {
        match self {
            Self::Split(SplitAxis::Columns) => "split the pane side by side",
            Self::Split(SplitAxis::Rows) => "split the pane top and bottom",
            Self::ClosePane => "close the pane",
            Self::Focus(_) => "focus the pane in that direction",
            Self::FocusNext => "focus the next pane",
            Self::FocusPrev => "focus the previous pane",
            Self::Swap(_) => "swap with the pane in that direction",
            Self::SwapNext => "swap with the next pane",
            Self::SwapPrev => "swap with the previous pane",
            Self::Resize(_) => "move the divider that way",
            Self::ToggleZoom => "zoom the pane to fill the tab, or unzoom",
            Self::Equalize => "give the tab's panes equal space",
            Self::NewTab => "open a tab",
            Self::NextTab => "show the next tab",
            Self::PrevTab => "show the previous tab",
            Self::SelectTab(_) => "show that tab",
            Self::Detach => "detach; the session keeps running",
            Self::ToggleMouse => "give the mouse to your terminal, or take it back",
            Self::CommandPalette => "search every command by name",
            Self::GoToPane => "jump to any pane by its title",
            Self::CopyMode => "scroll, search, select and copy the pane's history",
            Self::ToggleSidebar => "show or hide the sidebar",
            Self::ReloadConfig => "read the config file again",
            Self::Inbox => "agents waiting for you, done or failed",
            Self::NextBlocked => "jump to the next agent waiting for you",
            Self::Review => "review the checkout's uncommitted changes",
            Self::NewWorktree => "new worktree with an agent or a shell",
            Self::Projects => "jump to, open or remove worktrees",
            Self::Sessions => "switch to another session, or start one",
            Self::Help => "this help",
            Self::FocusPane(_) => "focus a pane",
        }
    }

    /// The area it belongs to.
    #[must_use]
    pub fn group(self) -> ActionGroup {
        match self {
            Self::Split(_)
            | Self::ClosePane
            | Self::Focus(_)
            | Self::FocusNext
            | Self::FocusPrev
            | Self::Swap(_)
            | Self::SwapNext
            | Self::SwapPrev
            | Self::Resize(_)
            | Self::ToggleZoom
            | Self::Equalize
            | Self::GoToPane
            | Self::CopyMode
            | Self::FocusPane(_) => ActionGroup::Panes,
            Self::NewTab | Self::NextTab | Self::PrevTab | Self::SelectTab(_) => ActionGroup::Tabs,
            Self::Inbox | Self::NextBlocked => ActionGroup::Agents,
            Self::Review | Self::NewWorktree | Self::Projects => ActionGroup::Projects,
            Self::Detach
            | Self::ToggleMouse
            | Self::CommandPalette
            | Self::ToggleSidebar
            | Self::ReloadConfig
            | Self::Sessions
            | Self::Help => ActionGroup::Session,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_a_distinct_summary_but_directions_share_one() {
        let all = Action::catalogue();
        let mut seen = std::collections::HashMap::new();
        for action in &all {
            let summary = action.summary();
            assert!(!summary.is_empty() && summary.len() < 60, "{action}");
            if let Some(other) = seen.insert(summary, *action) {
                assert_eq!(
                    std::mem::discriminant(&other),
                    std::mem::discriminant(action),
                    "{other} and {action} read the same"
                );
            }
        }
    }

    #[test]
    fn actions_fall_into_their_area() {
        assert_eq!(Action::Inbox.group(), ActionGroup::Agents);
        assert_eq!(Action::SelectTab(3).group(), ActionGroup::Tabs);
        assert_eq!(Action::Help.group(), ActionGroup::Session);
        assert!(
            ActionGroup::ALL
                .iter()
                .all(|g| Action::catalogue().iter().any(|a| a.group() == *g)),
            "no empty group"
        );
    }
}
