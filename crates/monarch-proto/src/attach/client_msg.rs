//! Messages from an attached client to the server.

use monarch_core::{Action, PaneId};
use serde::{Deserialize, Serialize};

use super::{HostInfo, SelectShape};
use crate::{GridPoint, NewWorktree, ScreenSize};

/// One client → server attach-stream message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ClientMsg {
    /// Bytes for a pane's program, already filtered for that pane.
    Input {
        /// Target pane.
        pane: PaneId,
        /// Exact bytes to write to its PTY.
        bytes: Vec<u8>,
    },
    /// The client's window size changed.
    Resize(ScreenSize),
    /// What the client learned by probing its host terminal.
    HostInfo(HostInfo),
    /// Move input focus to a pane.
    Focus(PaneId),
    /// Scroll a pane's view: positive into history, negative toward live.
    Scroll {
        /// Target pane.
        pane: PaneId,
        /// Lines to scroll.
        lines: i32,
    },
    /// Scroll a pane so absolute line `top` (0 = oldest scrollback line)
    /// is the first one shown, as near as history allows.
    ScrollTo {
        /// Target pane.
        pane: PaneId,
        /// The line to show at the top.
        top: i64,
    },
    /// The client lost track of a pane (sequence gap); send a snapshot.
    Resync(PaneId),
    /// Round-trip probe; the server answers with `Pong` and the same token.
    Ping(u64),
    /// The client is leaving; the session keeps running.
    Detach,
    /// Run a session command (split, focus, tabs…).
    Action(Action),
    /// Find `query` in a pane's screen and scrollback, starting just past
    /// `from` and wrapping around; answered with `ServerMsg::SearchResult`.
    Search {
        /// Pane to search.
        pane: PaneId,
        /// Plain text (not a pattern); case-insensitive unless it has
        /// capitals.
        query: String,
        /// Where the search starts (the copy-mode cursor).
        from: GridPoint,
        /// Search towards older output.
        backward: bool,
    },
    /// Copy a range of a pane's grid, scrollback included, to this
    /// client's clipboard; answered with `ServerMsg::Clipboard`.
    Copy {
        /// Pane to copy from.
        pane: PaneId,
        /// One end of the range.
        start: GridPoint,
        /// The other end, inclusive.
        end: GridPoint,
        /// How the range becomes text.
        shape: SelectShape,
    },
    /// Read the uncommitted changes in the checkout a pane works in;
    /// answered with `ServerMsg::Review`.
    Review(PaneId),
    /// Open a file from a pane's checkout in an editor, in a pane zoomed
    /// over the focused one; closing the editor gives the layout back.
    Edit {
        /// The pane whose checkout holds the file.
        pane: PaneId,
        /// The file, relative to the checkout's top directory.
        path: String,
        /// The editor command, run by `sh` with the file as its last
        /// argument (as git runs `$EDITOR`).
        editor: String,
    },
    /// Make a worktree and open a tab in it (as the API's
    /// `worktree.create`); answered with `ServerMsg::WorktreeMade`.
    NewWorktree(NewWorktree),
    /// List the worktrees of every repository the session's panes work
    /// in; answered with `ServerMsg::Worktrees`.
    Worktrees,
    /// Open a tab with a shell in this worktree directory.
    OpenWorktree(String),
    /// Remove this worktree (its branch stays); answered with
    /// `ServerMsg::WorktreeRemoved`.
    RemoveWorktree(String),
    /// Name the tab holding `pane`; a blank name gives the automatic one
    /// (its focused pane's) back.
    RenameTab {
        /// Any pane in the tab.
        pane: PaneId,
        /// The new name.
        name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_keeps_bytes_verbatim() {
        let msg = ClientMsg::Input {
            pane: PaneId(1),
            bytes: b"\x1b[A".to_vec(),
        };
        assert!(matches!(msg, ClientMsg::Input { ref bytes, .. } if bytes == b"\x1b[A"));
    }

    #[test]
    fn renames_survive_the_attach_encoding() {
        let msg = ClientMsg::RenameTab {
            pane: PaneId(2),
            name: "api".into(),
        };
        let bytes = postcard::to_allocvec(&msg).expect("encode");
        assert_eq!(postcard::from_bytes::<ClientMsg>(&bytes).ok(), Some(msg));
    }
}
