//! Works out which git project, checkout and branch a directory is in.

use std::path::Path;

use monarch_proto::PanePlace;

use crate::GitRunner;

/// Asks git where a directory is (two or three quick `git` runs).
pub struct RepoProbe {
    git: Box<dyn GitRunner>,
}

impl RepoProbe {
    /// A probe asking `git`.
    #[must_use]
    pub fn new(git: Box<dyn GitRunner>) -> Self {
        Self { git }
    }

    /// Where `cwd` is; outside a repository only the directory is known.
    #[must_use]
    pub fn place(&self, cwd: &Path) -> PanePlace {
        let mut place = PanePlace::outside(cwd.to_string_lossy());
        let args = [
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ];
        let Some(dirs) = self.git.run(cwd, &args) else {
            return place;
        };
        let mut lines = dirs.lines();
        let (Some(top), Some(common)) = (lines.next(), lines.next()) else {
            return place;
        };
        place.project = project_name(Path::new(common));
        place.worktree = Some(top.to_owned());
        place.branch = self
            .git
            .run(cwd, &["symbolic-ref", "--short", "-q", "HEAD"])
            .or_else(|| self.git.run(cwd, &["rev-parse", "--short", "HEAD"]))
            .filter(|b| !b.is_empty());
        place
    }
}

/// The repository's name from its shared git directory: `/w/monarch/.git`
/// and a bare `/w/monarch.git` are both `monarch`.
fn project_name(common: &Path) -> Option<String> {
    let name = common.file_name()?.to_string_lossy();
    let name = if name == ".git" {
        common.parent()?.file_name()?.to_string_lossy()
    } else {
        name
    };
    Some(name.strip_suffix(".git").unwrap_or(&name).to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// Answers each command line from a table.
    struct Scripted(BTreeMap<&'static str, &'static str>);

    impl GitRunner for Scripted {
        fn run(&self, _dir: &Path, args: &[&str]) -> Option<String> {
            self.0.get(args.join(" ").as_str()).map(|s| (*s).to_owned())
        }
    }

    const DIRS: &str = "rev-parse --path-format=absolute --show-toplevel --git-common-dir";
    const BRANCH: &str = "symbolic-ref --short -q HEAD";

    fn probe(answers: &[(&'static str, &'static str)]) -> RepoProbe {
        RepoProbe::new(Box::new(Scripted(answers.iter().copied().collect())))
    }

    #[test]
    fn a_worktree_belongs_to_its_main_repository() {
        let p = probe(&[
            (DIRS, "/w/monarch-feat\n/w/monarch/.git"),
            (BRANCH, "feat/sidebar"),
        ]);
        let place = p.place(Path::new("/w/monarch-feat/src"));
        assert_eq!(place.cwd, "/w/monarch-feat/src");
        assert_eq!(place.project.as_deref(), Some("monarch"));
        assert_eq!(place.worktree.as_deref(), Some("/w/monarch-feat"));
        assert_eq!(place.branch.as_deref(), Some("feat/sidebar"));
    }

    #[test]
    fn detached_heads_show_a_commit_and_bare_repos_lose_the_suffix() {
        let p = probe(&[
            (DIRS, "/w/api\n/w/api.git"),
            ("rev-parse --short HEAD", "1a2b3c4"),
        ]);
        let place = p.place(Path::new("/w/api"));
        assert_eq!(place.project.as_deref(), Some("api"));
        assert_eq!(place.branch.as_deref(), Some("1a2b3c4"));
    }

    #[test]
    fn outside_a_repository_only_the_directory_is_known() {
        assert_eq!(
            probe(&[]).place(Path::new("/tmp")),
            PanePlace::outside("/tmp")
        );
        let odd = probe(&[(DIRS, "/w/half")]);
        assert_eq!(odd.place(Path::new("/w")).project, None, "short answer");
    }
}
