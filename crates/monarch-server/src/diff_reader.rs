//! Reads a checkout's uncommitted changes for the review pane: staged and
//! unstaged edits against `HEAD`, plus untracked files shown as added.

mod unified_diff;

use std::io::Read;
use std::path::Path;

use monarch_proto::{DiffLine, DiffLineKind, FileChange, Review, ReviewFile};

use crate::GitRunner;

/// Git's empty tree: the base when nothing has been committed yet.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
/// Most diff lines sent; past this the review is marked truncated.
const MAX_LINES: usize = 20_000;
/// Most of an untracked file read for display.
const MAX_UNTRACKED: u64 = 64 * 1024;

/// Asks git what changed in a checkout.
pub struct DiffReader {
    git: Box<dyn GitRunner>,
}

impl DiffReader {
    /// A reader asking `git`.
    #[must_use]
    pub fn new(git: Box<dyn GitRunner>) -> Self {
        Self { git }
    }

    /// Every change in the checkout at `root`, in path order.
    #[must_use]
    pub fn review(&self, root: &Path) -> Review {
        let committed = self.git.run(root, &["rev-parse", "--verify", "-q", "HEAD"]);
        let base = if committed.is_some() {
            "HEAD"
        } else {
            EMPTY_TREE
        };
        let args = [
            "-c",
            "core.quotepath=off",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--find-renames",
            base,
        ];
        let mut files = unified_diff::parse(&self.git.run(root, &args).unwrap_or_default());
        let args = [
            "-c",
            "core.quotepath=off",
            "ls-files",
            "--others",
            "--exclude-standard",
        ];
        let untracked = self.git.run(root, &args).unwrap_or_default();
        files.extend(untracked.lines().map(|path| untracked_file(root, path)));
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let truncated = cap(&mut files);
        Review {
            root: root.to_string_lossy().into_owned(),
            branch: None,
            files,
            truncated,
        }
    }
}

/// An untracked file, its text shown as added lines.
fn untracked_file(root: &Path, path: &str) -> ReviewFile {
    let mut file = ReviewFile::new(path, FileChange::Untracked);
    let mut bytes = Vec::new();
    let read = std::fs::File::open(root.join(path))
        .and_then(|f| f.take(MAX_UNTRACKED).read_to_end(&mut bytes));
    if read.is_err() || bytes.contains(&0) {
        let note = if read.is_err() {
            "Unreadable"
        } else {
            "Binary file"
        };
        file.lines.push(DiffLine::new(DiffLineKind::Note, note));
        return file;
    }
    for line in String::from_utf8_lossy(&bytes).lines() {
        file.lines.push(DiffLine::new(DiffLineKind::Added, line));
    }
    file.added = u32::try_from(file.lines.len()).unwrap_or(u32::MAX);
    file
}

/// Keeps at most [`MAX_LINES`] diff lines; whether any were dropped.
fn cap(files: &mut [ReviewFile]) -> bool {
    let mut left = MAX_LINES;
    let mut dropped = false;
    for file in files {
        if file.lines.len() > left {
            file.lines.truncate(left);
            dropped = true;
        }
        left -= file.lines.len();
    }
    dropped
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// Answers each git command by its first distinctive word.
    struct Scripted {
        head: bool,
        diff: &'static str,
        untracked: &'static str,
        asked: Mutex<Vec<String>>,
    }

    impl GitRunner for Scripted {
        fn run(&self, _dir: &Path, args: &[&str]) -> Option<String> {
            self.asked.lock().expect("lock").push(args.join(" "));
            match args {
                ["rev-parse", ..] => self.head.then(|| "abc".to_owned()),
                [.., "diff", _, _, _, _] => Some(self.diff.to_owned()),
                [.., "ls-files", _, _] => Some(self.untracked.to_owned()),
                _ => None,
            }
        }
    }

    /// Lets the test keep a handle on what the reader asked.
    struct Shared(Arc<Scripted>);

    impl GitRunner for Shared {
        fn run(&self, dir: &Path, args: &[&str]) -> Option<String> {
            self.0.run(dir, args)
        }
    }

    fn reader(head: bool, untracked: &'static str) -> (DiffReader, Arc<Scripted>) {
        let git = Arc::new(Scripted {
            head,
            diff: "diff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -1 +1 @@\n-x\n+y\n",
            untracked,
            asked: Mutex::new(Vec::new()),
        });
        (DiffReader::new(Box::new(Shared(Arc::clone(&git)))), git)
    }

    #[test]
    fn tracked_and_untracked_changes_in_path_order() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").expect("write");
        std::fs::write(dir.path().join("c.bin"), b"\0\x01").expect("write");
        let (reader, git) = reader(true, "a.txt\nc.bin\nmissing.txt");
        let review = reader.review(dir.path());
        let shape: Vec<(&str, FileChange, u32, u32)> = review
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.change, f.added, f.removed))
            .collect();
        assert_eq!(
            shape,
            [
                ("a.txt", FileChange::Untracked, 2, 0),
                ("b.rs", FileChange::Modified, 1, 1),
                ("c.bin", FileChange::Untracked, 0, 0),
                ("missing.txt", FileChange::Untracked, 0, 0),
            ]
        );
        assert_eq!(review.files[2].lines[0].text, "Binary file");
        assert_eq!(review.files[3].lines[0].text, "Unreadable");
        assert!(!review.truncated);
        assert!(
            git.asked
                .lock()
                .expect("lock")
                .iter()
                .any(|a| a.ends_with(" HEAD"))
        );
    }

    #[test]
    fn a_repository_without_commits_diffs_against_the_empty_tree() {
        let (reader, git) = reader(false, "");
        let _ = reader.review(Path::new("/w"));
        assert!(
            git.asked
                .lock()
                .expect("lock")
                .iter()
                .any(|a| a.ends_with(EMPTY_TREE))
        );
    }

    #[test]
    fn huge_diffs_are_capped() {
        let mut big = ReviewFile::new("big", FileChange::Modified);
        big.lines = vec![DiffLine::new(DiffLineKind::Added, "x"); MAX_LINES + 5];
        let mut files = vec![big, ReviewFile::new("small", FileChange::Modified)];
        files[1].lines.push(DiffLine::new(DiffLineKind::Added, "y"));
        assert!(cap(&mut files));
        assert_eq!((files[0].lines.len(), files[1].lines.len()), (MAX_LINES, 0));
        assert!(!cap(&mut files), "already within the cap");
    }
}
