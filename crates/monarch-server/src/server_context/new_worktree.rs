//! `worktree.create`: a new worktree and a tab working in it.

use monarch_proto::{NewWorktree, WorktreeCreated};

use super::ServerContext;
use crate::{PaneSpec, Placement, ServerError};

impl ServerContext {
    /// Adds the worktree `request` asks for and opens a tab there running
    /// its agent (or a shell), shown at once. Runs git: call it off the
    /// async runtime.
    ///
    /// # Errors
    /// The worktree could not be made, or the pane could not start (the
    /// worktree then stays).
    pub fn new_worktree(&self, request: &NewWorktree) -> Result<WorktreeCreated, ServerError> {
        let (path, new_branch) = self.worktrees.create(request)?;
        let spec = PaneSpec {
            cwd: Some(path.clone()),
            ..request.agent.map(PaneSpec::agent).unwrap_or_default()
        };
        let pane = self.spawn(&spec, None, Placement::Tab { activate: true })?;
        Ok(WorktreeCreated {
            path: path.to_string_lossy().into_owned(),
            branch: request.name.clone(),
            new_branch,
            pane,
        })
    }
}
