//! Reads `git status --porcelain=v2 --branch` and, when tracked files
//! changed, `git diff HEAD --numstat`: the branch checked out and how the
//! checkout differs from its last commit (in lines) and its upstream.

use std::path::Path;

use monarch_proto::CheckoutStatus;

use crate::GitRunner;

/// One `git status` of a checkout. Pure data once read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GitStatus {
    /// The branch checked out; `None` when detached.
    pub head: Option<String>,
    /// Uncommitted lines and files, and commits ahead of or behind
    /// upstream.
    pub status: CheckoutStatus,
    /// Tracked files with changes: their lines need a second command.
    tracked: u32,
}

impl GitStatus {
    /// Asks git in `dir` (one run, two when tracked files changed; takes
    /// no locks); `None` outside a repository or when git fails.
    pub fn read(git: &dyn GitRunner, dir: &Path) -> Option<Self> {
        let mut read = Self::parse(&git.run(dir, &["status", "--porcelain=v2", "--branch"])?);
        if read.tracked > 0 {
            // Before the first commit there is no HEAD: staged is all there is.
            let numstat = git
                .run(dir, &["diff", "HEAD", "--numstat"])
                .or_else(|| git.run(dir, &["diff", "--cached", "--numstat"]));
            (read.status.added, read.status.removed) = numstat.as_deref().map_or((0, 0), lines);
        }
        Some(read)
    }

    /// `# branch.head` and `# branch.ab` headers, then one line per
    /// changed, unmerged or untracked file.
    fn parse(out: &str) -> Self {
        let mut read = Self::default();
        for line in out.lines() {
            if let Some(head) = line.strip_prefix("# branch.head ") {
                read.head = (head != "(detached)").then(|| head.to_owned());
            } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
                let count = |n: &str| n.get(1..).and_then(|d| d.parse().ok());
                let mut counts = ab.split(' ').map(|n| count(n).unwrap_or(0));
                read.status.ahead = counts.next().unwrap_or(0);
                read.status.behind = counts.next().unwrap_or(0);
            } else if line.starts_with("? ") {
                read.status.untracked = read.status.untracked.saturating_add(1);
            } else if !line.starts_with('#') && !line.is_empty() {
                read.tracked = read.tracked.saturating_add(1);
            }
        }
        read
    }
}

/// Lines added and removed in `--numstat` output (`12\t4\tpath`); a
/// binary file (`-\t-\tpath`) counts none.
fn lines(numstat: &str) -> (u32, u32) {
    numstat.lines().fold((0, 0), |(added, removed), line| {
        let mut counts = line.split('\t').map(|n| n.parse::<u32>().unwrap_or(0));
        let (a, r) = (counts.next().unwrap_or(0), counts.next().unwrap_or(0));
        (added.saturating_add(a), removed.saturating_add(r))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "# branch.oid 1234abcd\n# branch.head feat/x\n\
        # branch.upstream origin/feat/x\n# branch.ab +2 -1\n\
        1 .M N... 100644 100644 100644 aa bb src/a.rs\n\
        2 R. N... 100644 100644 100644 aa bb R100 b.rs\ta.rs\n\
        u UU N... 100644 100644 100644 100644 aa bb cc c.rs\n? new.txt";

    /// Answers `status` with `DIRTY` and `diff` from `numstat`.
    struct Git(Option<&'static str>);

    impl GitRunner for Git {
        fn run(&self, _dir: &Path, args: &[&str]) -> Option<String> {
            match args {
                ["status", ..] => Some(DIRTY.into()),
                ["diff", "HEAD", ..] => self.0.map(Into::into),
                ["diff", "--cached", ..] => Some("5\t0\ta.rs".into()),
                _ => None,
            }
        }
    }

    #[test]
    fn counts_lines_untracked_files_and_reads_branch_and_upstream() {
        let numstat = "10\t2\tsrc/a.rs\n2\t2\tb.rs\n-\t-\tlogo.png";
        let read = GitStatus::read(&Git(Some(numstat)), Path::new("/w")).expect("read");
        assert_eq!(read.head.as_deref(), Some("feat/x"));
        let status = CheckoutStatus {
            added: 12,
            removed: 4,
            untracked: 1,
            ahead: 2,
            behind: 1,
        };
        assert_eq!(read.status, status);
    }

    #[test]
    fn before_the_first_commit_staged_lines_count() {
        let read = GitStatus::read(&Git(None), Path::new("/w")).expect("read");
        assert_eq!((read.status.added, read.status.removed), (5, 0));
    }

    #[test]
    fn clean_detached_and_without_upstream() {
        let read = GitStatus::parse("# branch.oid 1234abcd\n# branch.head (detached)");
        assert_eq!(read, GitStatus::default());
        let read = GitStatus::parse("# branch.oid (initial)\n# branch.head main\n? a");
        assert_eq!(
            (read.head.as_deref(), read.status.untracked, read.tracked),
            (Some("main"), 1, 0)
        );
        assert_eq!((read.status.ahead, read.status.behind), (0, 0));
    }

    #[test]
    fn a_garbled_count_reads_as_zero() {
        for garbled in ["# branch.ab +x -", "# branch.ab "] {
            let read = GitStatus::parse(garbled);
            assert_eq!((read.status.ahead, read.status.behind), (0, 0));
        }
    }
}
