//! The server's answers to this client's own requests (reviews,
//! worktrees, explanations, quotes, files), each handed to the pop-up
//! that asked.

use super::AttachApp;
use crate::Applied;

impl AttachApp {
    /// Hands each answer in `applied` to whatever asked for it.
    pub(super) fn after_replies(&mut self, applied: &mut Applied) {
        for (pane, review) in std::mem::take(&mut applied.reviews) {
            self.loaded_review(pane, review);
        }
        for made in std::mem::take(&mut applied.worktrees) {
            self.worktree_made(made);
        }
        for list in std::mem::take(&mut applied.worktree_lists) {
            self.loaded_worktrees(list);
        }
        for removed in std::mem::take(&mut applied.removed) {
            self.worktree_removed(removed);
        }
        for (pane, explained) in std::mem::take(&mut applied.explained) {
            self.explained(pane, explained);
        }
        for (pane, text) in std::mem::take(&mut applied.quotes) {
            self.quoted(pane, text);
        }
        for (pane, files) in std::mem::take(&mut applied.files) {
            self.listed_files(pane, files);
        }
        for (pane, file) in std::mem::take(&mut applied.opened) {
            self.opened_file(pane, file);
        }
    }
}
