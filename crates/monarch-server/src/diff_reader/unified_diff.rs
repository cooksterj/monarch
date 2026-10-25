//! Parses `git diff` output into files and diff lines.

use monarch_proto::{DiffLine, DiffLineKind, FileChange, ReviewFile};

/// Every file in `text`, a `git diff` with `core.quotepath=off`.
pub(super) fn parse(text: &str) -> Vec<ReviewFile> {
    let mut files: Vec<ReviewFile> = Vec::new();
    let mut in_hunk = false;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("diff --git ") {
            files.push(ReviewFile::new(
                path_from_header(header),
                FileChange::Modified,
            ));
            in_hunk = false;
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if in_hunk && add_hunk_line(file, line) {
            continue;
        }
        in_hunk = header_line(file, line);
    }
    files
}

/// Applies a line in a file's header; whether it started a hunk.
fn header_line(file: &mut ReviewFile, line: &str) -> bool {
    if line.starts_with("@@") {
        file.lines.push(DiffLine::new(DiffLineKind::Hunk, line));
        return true;
    }
    if line.starts_with("new file mode") {
        file.change = FileChange::Added;
    } else if line.starts_with("deleted file mode") {
        file.change = FileChange::Deleted;
    } else if let Some(to) = line.strip_prefix("rename to ") {
        file.change = FileChange::Renamed;
        to.clone_into(&mut file.path);
    } else if let Some(path) = line.strip_prefix("+++ b/") {
        path.clone_into(&mut file.path);
    } else if line.starts_with("Binary files") {
        file.lines
            .push(DiffLine::new(DiffLineKind::Note, "Binary file"));
    }
    false
}

/// Adds a line inside a hunk; `false` when it isn't one (a new header).
fn add_hunk_line(file: &mut ReviewFile, line: &str) -> bool {
    let (kind, text) = match line.split_at_checked(1) {
        Some(("+", text)) => (DiffLineKind::Added, text),
        Some(("-", text)) => (DiffLineKind::Removed, text),
        Some((" ", text)) => (DiffLineKind::Context, text),
        Some(("\\", text)) => (DiffLineKind::Note, text.trim()),
        None => (DiffLineKind::Context, ""),
        _ => return false,
    };
    match kind {
        DiffLineKind::Added => file.added += 1,
        DiffLineKind::Removed => file.removed += 1,
        _ => {}
    }
    file.lines.push(DiffLine::new(kind, text));
    true
}

/// The new path in `a/<old> b/<new>`; the `+++` or `rename to` line,
/// when there is one, replaces it (paths may contain ` b/`).
fn path_from_header(header: &str) -> String {
    match header.rfind(" b/") {
        Some(i) => header[i + 3..].to_owned(),
        None => header.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 1111111..2222222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!(\"hi\");
+    println!(\"hello\");
+    println!(\"world\");

 }
\\ No newline at end of file
diff --git a/old.txt b/new.txt
similarity index 90%
rename from old.txt
rename to new.txt
diff --git a/gone.rs b/gone.rs
deleted file mode 100644
index 3333333..0000000
--- a/gone.rs
+++ /dev/null
@@ -1 +0,0 @@
-pub fn gone() {}
diff --git a/logo.png b/logo.png
new file mode 100644
index 0000000..4444444
Binary files /dev/null and b/logo.png differ
";

    #[test]
    fn files_lines_and_counts() {
        let files = parse(DIFF);
        let shape: Vec<(&str, FileChange, u32, u32)> = files
            .iter()
            .map(|f| (f.path.as_str(), f.change, f.added, f.removed))
            .collect();
        assert_eq!(
            shape,
            [
                ("src/main.rs", FileChange::Modified, 2, 1),
                ("new.txt", FileChange::Renamed, 0, 0),
                ("gone.rs", FileChange::Deleted, 0, 1),
                ("logo.png", FileChange::Added, 0, 0),
            ]
        );
        let main = &files[0].lines;
        assert_eq!(
            main[0],
            DiffLine::new(DiffLineKind::Hunk, "@@ -1,3 +1,4 @@")
        );
        assert_eq!(
            main[2],
            DiffLine::new(DiffLineKind::Removed, "    println!(\"hi\");")
        );
        assert_eq!(
            main[5],
            DiffLine::new(DiffLineKind::Context, ""),
            "a blank context line"
        );
        assert_eq!(main[7].kind, DiffLineKind::Note);
        assert_eq!(
            files[3].lines,
            [DiffLine::new(DiffLineKind::Note, "Binary file")]
        );
    }

    #[test]
    fn odd_input() {
        assert!(parse("").is_empty());
        assert!(parse("not a diff\n+stray").is_empty());
        let spaced = parse("diff --git a/my b/file b/my b/file\n");
        assert_eq!(spaced[0].path, "file", "best guess until +++ says");
    }
}
