//! What applying server messages asks the client to do.

use monarch_core::PaneId;
use monarch_proto::{
    CodeFile, GridPoint, PaneExplanation, Review, ServerMsg, WorktreeCreated, WorktreeEntry,
};

/// Side effects collected by [`super::AttachState::apply`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Something visible changed.
    pub redraw: bool,
    /// Panes whose frames arrived out of sequence; ask for snapshots.
    pub resync: Vec<PaneId>,
    /// Bytes for the host terminal (bell).
    pub host_write: Vec<u8>,
    /// Text a pane copied; goes to the user's clipboard.
    pub clipboard: Vec<String>,
    /// Panes whose programs exited.
    pub exited: Vec<PaneId>,
    /// Answers to searches: the pane and where the match is, if anywhere.
    pub found: Vec<(PaneId, Option<(GridPoint, GridPoint)>)>,
    /// Answers to review requests: the pane and its checkout's changes.
    pub reviews: Vec<(PaneId, Option<Review>)>,
    /// Answers to new-worktree requests.
    pub worktrees: Vec<Result<WorktreeCreated, String>>,
    /// Lists of the session's worktrees.
    pub worktree_lists: Vec<Vec<WorktreeEntry>>,
    /// Answers to worktree removals.
    pub removed: Vec<Result<String, String>>,
    /// Answers to explain requests: the pane and why its agent is in its
    /// state (`None` for a pane that is gone).
    pub explained: Vec<(PaneId, Option<Box<PaneExplanation>>)>,
    /// File lists for the file picker, by pane.
    pub files: Vec<(PaneId, Result<Vec<String>, String>)>,
    /// Files opened in the viewer, by pane.
    pub opened: Vec<(PaneId, Result<CodeFile, String>)>,
    /// Files to show over a pane, asked for from its shell.
    pub viewed: Vec<(PaneId, CodeFile)>,
    /// Answers to quote requests: the pane and the range's text.
    pub quotes: Vec<(PaneId, String)>,
    /// Panes whose agent status changed.
    pub agents: Vec<PaneId>,
}

impl Applied {
    /// Keeps `msg` if it is an answer or a request for the host (bell,
    /// clipboard) rather than a change to the session; ignores the rest.
    pub fn collect(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Explained { pane, explained } => self.explained.push((pane, explained)),
            ServerMsg::HostWrite(bytes) => self.host_write.extend(bytes),
            ServerMsg::SearchResult { pane, found } => self.found.push((pane, found)),
            ServerMsg::Review { pane, review } => self.reviews.push((pane, review)),
            ServerMsg::WorktreeMade(made) => self.worktrees.push(made),
            ServerMsg::Worktrees(list) => self.worktree_lists.push(list),
            ServerMsg::WorktreeRemoved(removed) => self.removed.push(removed),
            ServerMsg::Clipboard(text) => self.clipboard.push(text),
            ServerMsg::Quote { pane, text } => self.quotes.push((pane, text)),
            ServerMsg::Files { pane, files } => self.files.push((pane, files)),
            ServerMsg::FileOpened { pane, file } => self.opened.push((pane, file)),
            ServerMsg::ViewFile { pane, file } => self.viewed.push((pane, file)),
            _ => {}
        }
    }

    /// Whether there is nothing to do.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_kept_and_other_messages_ignored() {
        let mut applied = Applied::default();
        applied.collect(ServerMsg::Pong(1));
        assert!(applied.is_empty());
        let pane = PaneId(2);
        applied.collect(ServerMsg::Files {
            pane,
            files: Ok(vec!["a.rs".into()]),
        });
        applied.collect(ServerMsg::Clipboard("x".into()));
        applied.collect(ServerMsg::ViewFile {
            pane,
            file: CodeFile::default(),
        });
        assert_eq!(applied.viewed, [(pane, CodeFile::default())]);
        assert_eq!(applied.files, [(pane, Ok(vec!["a.rs".to_owned()]))]);
        assert_eq!(applied.clipboard, ["x"]);
    }

    #[test]
    fn default_is_empty() {
        assert!(Applied::default().is_empty());
        let redraw = Applied {
            redraw: true,
            ..Applied::default()
        };
        assert!(!redraw.is_empty());
    }
}
