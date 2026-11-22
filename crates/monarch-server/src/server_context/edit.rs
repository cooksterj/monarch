//! Opening a file from a pane's checkout in the user's editor.

use std::path::PathBuf;

use monarch_core::PaneId;

use super::ServerContext;
use crate::{FileReader, PaneSpec, Placement, ServerError};

impl ServerContext {
    /// Opens `path` from `pane`'s checkout with `editor` (at `line`, when
    /// given), zoomed over the focused pane. Returns the new pane, or
    /// `None` when `pane` isn't in a checkout or `path` would leave it.
    ///
    /// # Errors
    /// The PTY or program could not be started.
    pub fn edit(
        &self,
        pane: PaneId,
        path: &str,
        editor: &str,
        line: Option<usize>,
    ) -> Result<Option<PaneId>, ServerError> {
        let spec = self.edit_spec(pane, path, editor);
        match spec.map(|s| match line {
            Some(line) => s.at_line(editor, line),
            None => s,
        }) {
            Some(spec) => self.spawn(&spec, None, Placement::Zoomed).map(Some),
            None => Ok(None),
        }
    }

    /// What [`Self::edit`] would run, if the file is inside the checkout.
    pub(super) fn edit_spec(&self, pane: PaneId, path: &str, editor: &str) -> Option<PaneSpec> {
        let root = self.lock().place(pane)?.worktree?;
        (FileReader::is_inside(path) && !editor.trim().is_empty())
            .then(|| PaneSpec::edit(editor, path, PathBuf::from(root)))
    }
}
