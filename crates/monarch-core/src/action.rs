//! Everything a key binding can do: the one list the keymap, the command
//! palette and the attach protocol share.

use serde::{Deserialize, Serialize};

mod names;
mod summary;

use crate::{Direction, PaneId, SplitAxis};

/// Cells a single resize step moves a divider.
pub const RESIZE_STEP: u16 = 2;

/// A command. Most act on the session and travel to the server; a few
/// only change the client (see [`Action::is_local`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Action {
    /// Split the focused pane; the new pane opens in its directory.
    Split(SplitAxis),
    /// Close the focused pane (its program is hung up).
    ClosePane,
    /// Focus the neighbouring pane.
    Focus(Direction),
    /// Focus the next pane in reading order.
    FocusNext,
    /// Focus the previous pane in reading order.
    FocusPrev,
    /// Swap the focused pane with its neighbour; focus moves with it.
    Swap(Direction),
    /// Swap the focused pane with the next pane in reading order.
    SwapNext,
    /// Swap the focused pane with the previous pane in reading order.
    SwapPrev,
    /// Move the focused pane's divider [`RESIZE_STEP`] cells.
    Resize(Direction),
    /// Make the focused pane fill its tab, or restore the layout.
    ToggleZoom,
    /// Share the tab's space evenly.
    Equalize,
    /// Open a tab with a shell in the focused pane's directory.
    NewTab,
    /// Show the next tab.
    NextTab,
    /// Show the previous tab.
    PrevTab,
    /// Show tab number `n`, counting from 1.
    SelectTab(u8),
    /// Leave the session running and exit the client.
    Detach,
    /// Hand the mouse to the host terminal, or take it back.
    ToggleMouse,
    /// Open the command palette: every action, searched by name.
    CommandPalette,
    /// Open the go-to picker: every pane in every tab, searched by title.
    GoToPane,
    /// Browse the focused pane's history with the keyboard: move, search,
    /// select and copy.
    CopyMode,
    /// Show or hide the sidebar of panes grouped by project.
    ToggleSidebar,
    /// Read the config file again and apply it (it is also re-read
    /// whenever it changes).
    ReloadConfig,
    /// Open the attention inbox: every agent that is blocked, done or
    /// failed, to jump to or answer.
    Inbox,
    /// Focus the next blocked agent, in tab order after the focused pane.
    NextBlocked,
    /// Review the uncommitted changes in the focused pane's checkout:
    /// changed files and their diffs.
    Review,
    /// Make a git worktree for a new or existing branch and open a tab in
    /// it, running an agent or a shell.
    NewWorktree,
    /// Open the project picker: every worktree of every repository the
    /// session works in, to jump to, open or remove.
    Projects,
    /// Open the session picker: switch to another running session, or
    /// start one by name.
    Sessions,
    /// Show every key, mouse gesture and command, searchable.
    Help,
    /// Focus this pane, showing its tab (what the go-to picker runs; not
    /// bindable).
    FocusPane(PaneId),
}

impl Action {
    /// Whether the client handles this itself instead of the server.
    #[must_use]
    pub fn is_local(self) -> bool {
        matches!(
            self,
            Self::Detach
                | Self::ToggleMouse
                | Self::CommandPalette
                | Self::GoToPane
                | Self::CopyMode
                | Self::ToggleSidebar
                | Self::ReloadConfig
                | Self::Inbox
                | Self::NextBlocked
                | Self::Review
                | Self::NewWorktree
                | Self::Projects
                | Self::Sessions
                | Self::Help
        )
    }

    /// Whether pressing its key again soon repeats it without the prefix.
    /// Only resizes and swaps: repeating focus moves would swallow the
    /// first keys typed into the pane just focused.
    #[must_use]
    pub fn is_repeatable(self) -> bool {
        matches!(
            self,
            Self::Resize(_) | Self::Swap(_) | Self::SwapNext | Self::SwapPrev
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_client_actions_are_local() {
        assert!(Action::Detach.is_local() && Action::ToggleMouse.is_local());
        assert!(Action::CommandPalette.is_local() && Action::GoToPane.is_local());
        assert!(Action::CopyMode.is_local());
        assert!(Action::ToggleSidebar.is_local());
        assert!(Action::ReloadConfig.is_local());
        assert!(Action::Inbox.is_local() && Action::NextBlocked.is_local());
        assert!(Action::Review.is_local());
        assert!(Action::NewWorktree.is_local());
        assert!(Action::Projects.is_local());
        assert!(Action::Sessions.is_local() && Action::Help.is_local());
        assert!(!Action::FocusPane(PaneId(1)).is_local());
        assert!(!Action::Split(SplitAxis::Rows).is_local());
        assert!(!Action::SelectTab(1).is_local());
    }

    #[test]
    fn resizes_and_swaps_repeat_but_focus_does_not() {
        assert!(Action::Resize(Direction::Left).is_repeatable());
        assert!(Action::SwapNext.is_repeatable());
        assert!(!Action::Focus(Direction::Left).is_repeatable());
        assert!(!Action::ClosePane.is_repeatable());
    }

    #[test]
    fn actions_serialise_by_name() {
        let json = serde_json::to_string(&Action::Focus(Direction::Left)).expect("json");
        assert_eq!(json, r#"{"focus":"left"}"#);
        let back: Action = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, Action::Focus(Direction::Left));
    }
}
