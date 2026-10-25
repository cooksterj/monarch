//! Adds git worktrees: finds the repository, picks the directory from
//! `[projects]`, then checks out an existing branch or makes a new one.

use std::path::{Path, PathBuf};

use monarch_core::ProjectsConfig;
use monarch_proto::NewWorktree;

use crate::{GitRunner, ServerError};

/// Creates worktrees through git.
pub struct WorktreeMaker {
    git: Box<dyn GitRunner>,
}

impl WorktreeMaker {
    /// A maker asking `git`.
    #[must_use]
    pub fn new(git: Box<dyn GitRunner>) -> Self {
        Self { git }
    }

    /// Adds the worktree `request` asks for; returns its directory and
    /// whether its branch was made for it.
    ///
    /// # Errors
    /// Not in a repository, a bad name, the directory exists, or git
    /// refused (its message is kept).
    pub fn create(&self, request: &NewWorktree) -> Result<(PathBuf, bool), ServerError> {
        let dir = Path::new(&request.dir);
        let name = request.name.as_str();
        let repo = self.repository(dir).ok_or_else(|| {
            ServerError::Worktree(format!("{} is not in a git repository", request.dir))
        })?;
        self.check(dir, request)?;
        let projects = ProjectsConfig {
            worktree_root: request.root.as_ref().map(PathBuf::from),
        };
        let path = projects.worktree_path(&repo, &name.replace('/', "-"));
        if path.exists() {
            return Err(ServerError::Worktree(format!(
                "{} already exists",
                path.display()
            )));
        }
        let branch = format!("refs/heads/{name}");
        let exists = self
            .git
            .run(dir, &["rev-parse", "--verify", "--quiet", &branch])
            .is_some();
        if exists && request.base.is_some() {
            return Err(ServerError::Worktree(format!(
                "branch {name} exists; a base only applies to a new branch"
            )));
        }
        let target = path.to_string_lossy();
        let mut args = vec!["worktree", "add"];
        if exists {
            args.extend([&*target, name]);
        } else {
            args.extend(["-b", name, &*target]);
            args.extend(request.base.as_deref());
        }
        self.git
            .try_run(dir, &args)
            .map_err(ServerError::Worktree)?;
        Ok((path, !exists))
    }

    /// Refuses names git wouldn't take, and anything that would read as
    /// an option.
    fn check(&self, dir: &Path, request: &NewWorktree) -> Result<(), ServerError> {
        let name = request.name.as_str();
        let dashed = |s: &str| s.starts_with('-');
        if dashed(name) || request.base.as_deref().is_some_and(dashed) {
            return Err(ServerError::Worktree(format!(
                "{name:?} is not a valid branch name"
            )));
        }
        self.git
            .try_run(dir, &["check-ref-format", "--branch", name])
            .map(drop)
            .map_err(|_| ServerError::Worktree(format!("{name:?} is not a valid branch name")))
    }

    /// The main checkout of the repository holding `dir` (worktrees made
    /// from a worktree still go beside the main one).
    fn repository(&self, dir: &Path) -> Option<PathBuf> {
        let common = self.git.run(
            dir,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let common = PathBuf::from(common);
        Some(match common.file_name() {
            Some(n) if n == ".git" => common.parent()?.to_path_buf(),
            _ => common,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// Answers like a repository at `/src/app` with branch `old`, and
    /// records every command.
    #[derive(Clone, Default)]
    struct FakeGit(Arc<Mutex<Vec<String>>>);

    impl GitRunner for FakeGit {
        fn run(&self, _dir: &Path, args: &[&str]) -> Option<String> {
            self.0.lock().expect("log").push(args.join(" "));
            match args {
                ["rev-parse", "--path-format=absolute", "--git-common-dir"] => {
                    Some("/src/app/.git".into())
                }
                ["rev-parse", "--verify", "--quiet", branch] => {
                    (*branch == "refs/heads/old").then(String::new)
                }
                ["check-ref-format", "--branch", name] => {
                    (!name.contains("..")).then(|| (*name).to_owned())
                }
                ["worktree", "add", ..] => Some(String::new()),
                _ => None,
            }
        }
    }

    fn request(name: &str) -> NewWorktree {
        NewWorktree {
            dir: "/src/app/sub".into(),
            name: name.into(),
            root: Some("/nonexistent-monarch-wt".into()),
            ..NewWorktree::default()
        }
    }

    #[test]
    fn new_branches_start_from_the_base() {
        let git = FakeGit::default();
        let maker = WorktreeMaker::new(Box::new(git.clone()));
        let made = maker
            .create(&NewWorktree {
                base: Some("main".into()),
                ..request("feat/x")
            })
            .expect("made");
        let path = PathBuf::from("/nonexistent-monarch-wt/app/feat-x");
        assert_eq!(made, (path, true));
        let log = git.0.lock().expect("log").clone();
        assert_eq!(
            log.last().map(String::as_str),
            Some("worktree add -b feat/x /nonexistent-monarch-wt/app/feat-x main")
        );
    }

    #[test]
    fn existing_branches_are_checked_out() {
        let git = FakeGit::default();
        let maker = WorktreeMaker::new(Box::new(git.clone()));
        let (_, new_branch) = maker.create(&request("old")).expect("made");
        assert!(!new_branch);
        let log = git.0.lock().expect("log").clone();
        assert_eq!(
            log.last().map(String::as_str),
            Some("worktree add /nonexistent-monarch-wt/app/old old")
        );
        let based = NewWorktree {
            base: Some("main".into()),
            ..request("old")
        };
        assert!(maker.create(&based).is_err(), "base for an existing branch");
    }

    #[test]
    fn bad_names_and_places_are_refused() {
        let maker = WorktreeMaker::new(Box::new(FakeGit::default()));
        for name in ["-f", "a..b"] {
            let err = maker.create(&request(name)).expect_err("refused");
            assert!(err.to_string().contains("not a valid branch name"), "{err}");
        }
        let root = std::env::temp_dir().join(format!("monarch-wt-{}", std::process::id()));
        std::fs::create_dir_all(root.join("app/taken")).expect("dir");
        let taken = NewWorktree {
            root: Some(root.to_string_lossy().into_owned()),
            ..request("taken")
        };
        let err = maker.create(&taken).expect_err("refused");
        assert!(err.to_string().contains("already exists"), "{err}");
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
