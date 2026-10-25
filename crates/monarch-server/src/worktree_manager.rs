//! A repository's git worktrees: lists them, adds one (the directory comes
//! from `[projects]`; an existing branch is checked out, else a new one is
//! made) and removes one.

use std::path::{Path, PathBuf};

use monarch_core::ProjectsConfig;
use monarch_proto::{CheckoutStatus, NewWorktree};

use crate::{GitRunner, GitStatus, ServerError};

/// Lists, creates and removes worktrees through git, and reads their
/// status.
pub struct WorktreeManager {
    git: Box<dyn GitRunner>,
}

impl WorktreeManager {
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

    /// Every worktree of the repository holding `dir`, main checkout
    /// first, with its branch (`None` when detached). Bare and prunable
    /// (directory gone) entries are left out; outside a repository, none.
    #[must_use]
    pub fn list(&self, dir: &Path) -> Vec<(PathBuf, Option<String>)> {
        self.git
            .run(dir, &["worktree", "list", "--porcelain"])
            .map_or_else(Vec::new, |out| parse_list(&out))
    }

    /// Removes the worktree at `path`; its branch stays. Git refuses one
    /// with uncommitted or untracked changes.
    ///
    /// # Errors
    /// Not a worktree, the main checkout, or git refused (its message).
    pub fn remove(&self, path: &Path) -> Result<(), ServerError> {
        let main = self.repository(path).ok_or_else(|| {
            ServerError::Worktree(format!("{} is not a worktree", path.display()))
        })?;
        if main == path {
            return Err(ServerError::Worktree(
                "the main checkout can't be removed".into(),
            ));
        }
        let target = path.to_string_lossy();
        let name = path
            .file_name()
            .map_or(target.clone(), |n| n.to_string_lossy());
        self.git
            .try_run(&main, &["worktree", "remove", &target])
            .map(drop)
            .map_err(|why| {
                // Git suggests --force, which Monarch never uses.
                ServerError::Worktree(if why.contains("modified or untracked") {
                    format!("{name} has uncommitted changes; commit or discard them first")
                } else {
                    why
                })
            })
    }

    /// How the checkout at `path` differs from its last commit and
    /// upstream; `None` when git can't say.
    #[must_use]
    pub fn status(&self, path: &Path) -> Option<CheckoutStatus> {
        GitStatus::read(self.git.as_ref(), path).map(|r| r.status)
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

/// Reads `git worktree list --porcelain`: blocks of `worktree <path>`,
/// `HEAD …`, then `branch refs/heads/<name>` or `detached`, and maybe
/// `bare`, `locked` or `prunable`.
fn parse_list(out: &str) -> Vec<(PathBuf, Option<String>)> {
    let mut list = Vec::new();
    let mut skip = false;
    for line in out.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if skip {
                list.pop();
            }
            skip = false;
            list.push((PathBuf::from(path), None));
        } else if let (Some(branch), Some(last)) = (line.strip_prefix("branch "), list.last_mut()) {
            last.1 = Some(branch.trim_start_matches("refs/heads/").to_owned());
        } else if line == "bare" || line.starts_with("prunable") {
            skip = true;
        }
    }
    if skip {
        list.pop();
    }
    list
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
                ["worktree", "add" | "remove", ..] => Some(String::new()),
                _ => None,
            }
        }
    }

    /// Git refusing to remove a worktree with changes.
    struct Refuses;

    impl GitRunner for Refuses {
        fn run(&self, dir: &Path, args: &[&str]) -> Option<String> {
            FakeGit::default().run(dir, args)
        }

        fn try_run(&self, _dir: &Path, _args: &[&str]) -> Result<String, String> {
            Err(
                "'/w/app/feat-x' contains modified or untracked files, use --force to delete it"
                    .into(),
            )
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
        let maker = WorktreeManager::new(Box::new(git.clone()));
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
        let maker = WorktreeManager::new(Box::new(git.clone()));
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
    fn listing_skips_bare_and_vanished_worktrees() {
        let out = "worktree /src/app.git\nbare\n\n\
                   worktree /src/app\nHEAD 1\nbranch refs/heads/main\n\n\
                   worktree /w/app/feat-x\nHEAD 2\nbranch refs/heads/feat/x\n\n\
                   worktree /w/app/gone\nHEAD 3\nbranch refs/heads/gone\nprunable gitdir file points to non-existent location\n\n\
                   worktree /w/app/probe\nHEAD 4\ndetached\n";
        let list = parse_list(out);
        let shown: Vec<_> = list
            .iter()
            .map(|(p, b)| (p.to_string_lossy().into_owned(), b.clone()))
            .collect();
        assert_eq!(
            shown,
            [
                ("/src/app".to_owned(), Some("main".to_owned())),
                ("/w/app/feat-x".to_owned(), Some("feat/x".to_owned())),
                ("/w/app/probe".to_owned(), None),
            ]
        );
        assert_eq!(parse_list(""), []);
    }

    #[test]
    fn removal_runs_from_the_main_checkout_and_spares_it() {
        let git = FakeGit::default();
        let manager = WorktreeManager::new(Box::new(git.clone()));
        manager.remove(Path::new("/w/app/feat-x")).expect("removed");
        let log = git.0.lock().expect("log").clone();
        assert_eq!(
            log.last().map(String::as_str),
            Some("worktree remove /w/app/feat-x")
        );
        let err = manager.remove(Path::new("/src/app")).expect_err("main");
        assert!(err.to_string().contains("main checkout"), "{err}");
        let dirty = WorktreeManager::new(Box::new(Refuses));
        let err = dirty.remove(Path::new("/w/app/feat-x")).expect_err("dirty");
        assert_eq!(
            err.to_string(),
            "feat-x has uncommitted changes; commit or discard them first"
        );
    }

    #[test]
    fn bad_names_and_places_are_refused() {
        let maker = WorktreeManager::new(Box::new(FakeGit::default()));
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
