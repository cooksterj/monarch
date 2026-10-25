//! Reads `git status --porcelain=v2 --branch`: the branch checked out and
//! how the checkout differs from its last commit and its upstream.

use std::path::Path;

use monarch_proto::CheckoutStatus;

use crate::GitRunner;

/// One `git status` of a checkout. Pure data once read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GitStatus {
    /// The branch checked out; `None` when detached.
    pub head: Option<String>,
    /// Uncommitted files and commits ahead of or behind upstream.
    pub status: CheckoutStatus,
}

impl GitStatus {
    /// Asks git in `dir` (one run; takes no locks); `None` outside a
    /// repository or when git fails.
    pub fn read(git: &dyn GitRunner, dir: &Path) -> Option<Self> {
        git.run(dir, &["status", "--porcelain=v2", "--branch"])
            .map(|out| Self::parse(&out))
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
            } else if !line.starts_with('#') && !line.is_empty() {
                read.status.changed = read.status.changed.saturating_add(1);
            }
        }
        read
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRTY: &str = "# branch.oid 1234abcd\n# branch.head feat/x\n\
        # branch.upstream origin/feat/x\n# branch.ab +2 -1\n\
        1 .M N... 100644 100644 100644 aa bb src/a.rs\n\
        2 R. N... 100644 100644 100644 aa bb R100 b.rs\ta.rs\n\
        u UU N... 100644 100644 100644 100644 aa bb cc c.rs\n? new.txt";

    #[test]
    fn counts_files_and_reads_branch_and_upstream() {
        let read = GitStatus::parse(DIRTY);
        assert_eq!(read.head.as_deref(), Some("feat/x"));
        let status = CheckoutStatus {
            changed: 4,
            ahead: 2,
            behind: 1,
        };
        assert_eq!(read.status, status);
    }

    #[test]
    fn clean_detached_and_without_upstream() {
        let read = GitStatus::parse("# branch.oid 1234abcd\n# branch.head (detached)");
        assert_eq!(read, GitStatus::default());
        let read = GitStatus::parse("# branch.oid (initial)\n# branch.head main\n? a");
        assert_eq!(
            (read.head.as_deref(), read.status.changed),
            (Some("main"), 1)
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
