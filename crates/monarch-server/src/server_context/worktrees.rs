//! Worktrees for the session: listing those of every repository a pane
//! works in, making one with a tab in it, opening and removing one.

use std::path::{Path, PathBuf};

use monarch_core::PaneId;
use monarch_proto::{NewWorktree, WorktreeCreated, WorktreeEntry};

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

    /// Every worktree of every repository a pane works in: projects in
    /// the order their panes come, main checkout first. Runs git.
    #[must_use]
    pub fn worktree_list(&self) -> Vec<WorktreeEntry> {
        let checkouts = self.pane_checkouts();
        let mut out: Vec<WorktreeEntry> = Vec::new();
        for (_, dir) in &checkouts {
            if out.iter().any(|e| e.path == *dir) {
                continue;
            }
            let list = self.worktrees.list(Path::new(dir));
            let Some(project) = list.first().and_then(|(main, _)| main.file_name()) else {
                continue;
            };
            let project = project.to_string_lossy().into_owned();
            for (i, (path, branch)) in list.iter().enumerate() {
                let path = path.to_string_lossy().into_owned();
                let panes = checkouts.iter().filter(|(_, d)| *d == path);
                out.push(WorktreeEntry {
                    project: project.clone(),
                    branch: branch.clone(),
                    main: i == 0,
                    panes: panes.map(|(p, _)| *p).collect(),
                    path,
                });
            }
        }
        out
    }

    /// Opens a tab with a shell in the worktree at `path`, shown at once.
    ///
    /// # Errors
    /// `path` is not a directory, or the shell could not start.
    pub fn open_worktree(&self, path: &str) -> Result<PaneId, ServerError> {
        let dir = PathBuf::from(path);
        if !dir.is_dir() {
            return Err(ServerError::Worktree(format!("{path} is not a directory")));
        }
        let spec = PaneSpec {
            cwd: Some(dir),
            ..PaneSpec::default()
        };
        self.spawn(&spec, None, Placement::Tab { activate: true })
    }

    /// Removes the worktree at `path` unless a pane still works there.
    /// Runs git.
    ///
    /// # Errors
    /// Panes are there, or git refused (see [`crate::WorktreeManager`]).
    pub fn remove_worktree(&self, path: &str) -> Result<(), ServerError> {
        let there = self
            .pane_checkouts()
            .iter()
            .filter(|(_, d)| d == path)
            .count();
        if there > 0 {
            let s = if there == 1 { "" } else { "s" };
            return Err(ServerError::Worktree(format!(
                "close the {there} pane{s} working there first"
            )));
        }
        self.worktrees.remove(Path::new(path))
    }

    /// Each pane in a git checkout, with that checkout's directory.
    fn pane_checkouts(&self) -> Vec<(PaneId, String)> {
        let state = self.lock();
        state
            .pane_ids()
            .into_iter()
            .filter_map(|id| Some((id, state.place(id)?.worktree?)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::PanePlace;

    use super::*;
    use crate::test_support::{FakePane, context};
    use crate::{GitRunner, WorktreeManager};

    /// A repository at `/src/app` with a worktree at `/w/app/feat`.
    struct TwoWorktrees;

    impl GitRunner for TwoWorktrees {
        fn run(&self, _dir: &Path, args: &[&str]) -> Option<String> {
            match args {
                ["worktree", "list", "--porcelain"] => Some(
                    "worktree /src/app\nHEAD 1\nbranch refs/heads/main\n\n\
                     worktree /w/app/feat\nHEAD 2\nbranch refs/heads/feat\n"
                        .into(),
                ),
                ["rev-parse", ..] => Some("/src/app/.git".into()),
                ["worktree", "remove", ..] => Some(String::new()),
                _ => None,
            }
        }
    }

    fn in_checkout(ctx: &ServerContext, worktree: &str) -> PaneId {
        let id = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        let place = PanePlace {
            worktree: Some(worktree.into()),
            ..PanePlace::outside(worktree)
        };
        ctx.lock().set_place(id, Some(place));
        id
    }

    #[test]
    fn lists_every_worktree_with_its_panes() {
        let ctx = context().with_worktrees(WorktreeManager::new(Box::new(TwoWorktrees)));
        let a = in_checkout(&ctx, "/src/app");
        let b = in_checkout(&ctx, "/src/app");
        ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        let list = ctx.worktree_list();
        let shown: Vec<_> = list
            .iter()
            .map(|e| (e.project.as_str(), e.path.as_str(), e.main, e.panes.clone()))
            .collect();
        assert_eq!(
            shown,
            [
                ("app", "/src/app", true, vec![a, b]),
                ("app", "/w/app/feat", false, vec![]),
            ]
        );
    }

    #[test]
    fn worktrees_with_panes_are_not_removed() {
        let ctx = context().with_worktrees(WorktreeManager::new(Box::new(TwoWorktrees)));
        in_checkout(&ctx, "/w/app/feat");
        let err = ctx.remove_worktree("/w/app/feat").expect_err("pane there");
        assert!(
            err.to_string().contains("close the 1 pane working"),
            "{err}"
        );
        assert!(ctx.remove_worktree("/w/app/other").is_ok());
        assert!(ctx.open_worktree("/nonexistent-monarch").is_err());
    }
}
