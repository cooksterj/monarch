//! Messages from the server to an attached client.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use super::{AgentStatus, AgentTiming, Delta, LayoutMsg, PanePlace, Snapshot, StyleDef};
use crate::{CodeFile, GridPoint, PaneExplanation, Review, WorktreeCreated, WorktreeEntry};

/// One server → client attach-stream message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ServerMsg {
    /// The pane arrangement changed.
    Layout(LayoutMsg),
    /// A pane's full screen.
    Snapshot(Snapshot),
    /// A pane's changed rows.
    Delta(Delta),
    /// A new style id, valid from here on.
    DefineStyle(StyleDef),
    /// Forget every style id (the server's table started over).
    ResetStyles,
    /// A pane's program set or cleared its title.
    Title {
        /// The pane.
        pane: PaneId,
        /// The new title.
        title: Option<String>,
    },
    /// A pane's foreground program changed (`zsh` → `vim`); names the
    /// pane when its program sets no title.
    Command {
        /// The pane.
        pane: PaneId,
        /// The program's name.
        command: Option<String>,
    },
    /// Bytes for the host terminal itself (bell).
    HostWrite(Vec<u8>),
    /// A visible pane's program copied text (OSC 52); the client puts it
    /// on the user's clipboard. Reading the clipboard is never offered.
    Clipboard(String),
    /// A pane's program exited.
    PaneExited {
        /// The pane.
        pane: PaneId,
    },
    /// Answer to [`super::ClientMsg::Ping`], echoing its token.
    Pong(u64),
    /// Answer to [`super::ClientMsg::Search`]: the first and last cell of
    /// the match, or `None` when the text is nowhere in the pane.
    SearchResult {
        /// The pane searched.
        pane: PaneId,
        /// Where the match is.
        found: Option<(GridPoint, GridPoint)>,
    },
    /// Answer to [`super::ClientMsg::Review`]: the pane's checkout and its
    /// changes, or `None` outside a git repository.
    Review {
        /// The pane asked about.
        pane: PaneId,
        /// What changed.
        review: Option<Review>,
    },
    /// Answer to [`super::ClientMsg::NewWorktree`]: what was made, or why
    /// nothing was.
    WorktreeMade(Result<WorktreeCreated, String>),
    /// Answer to [`super::ClientMsg::Worktrees`]: by project, main
    /// checkout first in each.
    Worktrees(Vec<WorktreeEntry>),
    /// Answer to [`super::ClientMsg::RemoveWorktree`]: the directory
    /// removed, or why it wasn't.
    WorktreeRemoved(Result<String, String>),
    /// A pane's working directory or git checkout changed.
    Place {
        /// The pane.
        pane: PaneId,
        /// Where it is now.
        place: Option<PanePlace>,
    },
    /// A pane's agent state or activity changed.
    Agent {
        /// The pane.
        pane: PaneId,
        /// What its agent is doing now.
        status: AgentStatus,
        /// When its state and turn began and how long its last turn
        /// took; `None` for a plain shell.
        timing: Option<AgentTiming>,
    },
    /// Answer to [`super::ClientMsg::Explain`]: the pane's status, where
    /// it came from, its recent transitions and what its screen shows to
    /// its agent's manifest; `None` for a pane that is gone.
    Explained {
        /// The pane asked about.
        pane: PaneId,
        /// Why it is in its state.
        explained: Option<Box<PaneExplanation>>,
    },
    /// Answer to [`super::ClientMsg::ListFiles`]: the checkout's files,
    /// tracked and untracked but not ignored, or why there are none.
    Files {
        /// The pane asked about.
        pane: PaneId,
        /// Paths from the checkout's top, in order.
        files: Result<Vec<String>, String>,
    },
    /// Answer to [`super::ClientMsg::OpenFile`]: the file, highlighted,
    /// or why it can't be shown.
    FileOpened {
        /// The pane asked about.
        pane: PaneId,
        /// The file.
        file: Result<CodeFile, String>,
    },
    /// Answer to [`super::ClientMsg::Quote`]: the text of the range.
    Quote {
        /// The pane read.
        pane: PaneId,
        /// The range's text, wrapped lines whole.
        text: String,
    },
    /// Open the file viewer over `pane` on `file`: asked for from the
    /// pane's shell (`monarch view`), which every attached client shows,
    /// or answering [`super::ClientMsg::ViewSelected`].
    ViewFile {
        /// The pane to show it over.
        pane: PaneId,
        /// The file, highlighted.
        file: CodeFile,
        /// The line to open on (from 1), when the name gave one
        /// (`src/a.rs:42`).
        line: Option<usize>,
    },
    /// Answer to [`super::ClientMsg::ViewSelected`] when the selection
    /// names no file: its text, to search the checkout for instead.
    NotAFile {
        /// The pane it was selected in.
        pane: PaneId,
        /// What to search for: the path the text seems to be, else the
        /// text.
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_compare_by_content() {
        assert_eq!(ServerMsg::Pong(3), ServerMsg::Pong(3));
        assert_ne!(ServerMsg::ResetStyles, ServerMsg::HostWrite(vec![7]));
        assert_ne!(
            ServerMsg::Clipboard("a".into()),
            ServerMsg::Clipboard("b".into())
        );
    }

    #[test]
    fn file_answers_survive_the_attach_encoding() {
        let pane = PaneId(2);
        for msg in [
            ServerMsg::Files {
                pane,
                files: Ok(vec!["src/a.rs".into()]),
            },
            ServerMsg::Files {
                pane,
                files: Err("not in a git checkout".into()),
            },
            ServerMsg::FileOpened {
                pane,
                file: Ok(CodeFile::default()),
            },
            ServerMsg::ViewFile {
                pane,
                file: CodeFile::default(),
                line: Some(42),
            },
            ServerMsg::NotAFile {
                pane,
                text: "fn main".into(),
            },
        ] {
            let bytes = postcard::to_allocvec(&msg).expect("encode");
            assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).ok(), Some(msg));
        }
    }

    #[test]
    fn quotes_survive_the_attach_encoding() {
        let msg = ServerMsg::Quote {
            pane: PaneId(2),
            text: "fn main() {}\n".into(),
        };
        let bytes = postcard::to_allocvec(&msg).expect("encode");
        assert_eq!(postcard::from_bytes::<ServerMsg>(&bytes).ok(), Some(msg));
    }
}
