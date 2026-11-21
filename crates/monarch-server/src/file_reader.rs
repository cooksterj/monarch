//! Reads files from a checkout for the file viewer: the list of them, and
//! one file highlighted. Never reads outside the checkout.

use std::io::Read;
use std::path::{Component, Path};
use std::sync::Mutex;

use monarch_proto::CodeFile;
use monarch_syntax::{Highlighter, Language};

use crate::GitRunner;

/// Most files listed; a bigger checkout lists its first ones.
const MAX_FILES: usize = 50_000;
/// Most of a file read; the rest is left out.
const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// Most lines sent.
const MAX_LINES: usize = 20_000;
/// How much of a file is looked at to tell binary from text.
const SNIFF: usize = 8 * 1024;

/// Lists and reads a checkout's files, highlighting what it reads.
pub struct FileReader {
    git: Box<dyn GitRunner>,
    syntax: Mutex<Highlighter>,
}

impl FileReader {
    /// A reader asking `git` for file lists.
    #[must_use]
    pub fn new(git: Box<dyn GitRunner>) -> Self {
        Self {
            git,
            syntax: Mutex::new(Highlighter::default()),
        }
    }

    /// Whether `path` is a plain relative path that stays inside the
    /// directory it is joined to (no `..`, no root, not empty).
    #[must_use]
    pub fn is_inside(path: &str) -> bool {
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
    }

    /// The files in the checkout at `root`: tracked and untracked, not
    /// ignored, in git's order. Runs git: call it off the async runtime.
    ///
    /// # Errors
    /// Git failed.
    pub fn list(&self, root: &Path) -> Result<Vec<String>, String> {
        let args = [
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--deduplicate",
        ];
        let out = self.git.try_run(root, &args)?;
        let files = out.split('\0').filter(|p| !p.is_empty());
        Ok(files.take(MAX_FILES).map(str::to_owned).collect())
    }

    /// `path` in the checkout at `root`, highlighted by its language.
    /// Reads the disk and highlights: call it off the async runtime.
    ///
    /// # Errors
    /// The path leaves the checkout, can't be read, or is binary.
    pub fn open(&self, root: &Path, path: &str) -> Result<CodeFile, String> {
        let (text, cut) = read_text(root, path)?;
        let mut lines = {
            let mut syntax = self
                .syntax
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            syntax.highlight(Language::for_path(Path::new(path)), &text)
        };
        let truncated = cut || lines.len() > MAX_LINES;
        lines.truncate(MAX_LINES);
        Ok(CodeFile {
            path: path.to_owned(),
            language: Language::for_path(Path::new(path)).map(|l| l.name().to_owned()),
            lines,
            truncated,
        })
    }
}

/// The text of `path` under `root` (at most [`MAX_BYTES`], with whether
/// it was cut), refusing paths that leave `root`, symlinks included.
fn read_text(root: &Path, path: &str) -> Result<(String, bool), String> {
    if !FileReader::is_inside(path) {
        return Err(format!("{path} is outside the checkout"));
    }
    let failed = |e: std::io::Error| format!("can't read {path}: {e}");
    let real = root.join(path).canonicalize().map_err(failed)?;
    let top = root.canonicalize().map_err(failed)?;
    if !real.starts_with(&top) {
        return Err(format!("{path} links outside the checkout"));
    }
    let mut bytes = Vec::new();
    let file = std::fs::File::open(&real).map_err(failed)?;
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(failed)?;
    if bytes[..bytes.len().min(SNIFF)].contains(&0) {
        return Err(format!("{path} is a binary file"));
    }
    let cut = bytes.len() as u64 > MAX_BYTES;
    bytes.truncate(usize::try_from(MAX_BYTES).unwrap_or(usize::MAX));
    Ok((String::from_utf8_lossy(&bytes).into_owned(), cut))
}

#[cfg(test)]
mod tests {
    use monarch_core::SyntaxToken;

    use super::*;

    struct LsFiles;

    impl GitRunner for LsFiles {
        fn run(&self, _dir: &Path, args: &[&str]) -> Option<String> {
            (args[0] == "ls-files").then(|| "src/a.rs\0b c.txt\0".to_owned())
        }
    }

    fn checkout() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("src")).expect("mkdir");
        std::fs::write(dir.path().join("src/a.rs"), "fn a() {}\n").expect("write");
        std::fs::write(dir.path().join("bin"), b"\x7fELF\0\0").expect("write");
        dir
    }

    #[test]
    fn lists_what_git_lists() {
        let reader = FileReader::new(Box::new(LsFiles));
        let files = reader.list(Path::new("/w"));
        assert_eq!(files, Ok(vec!["src/a.rs".to_owned(), "b c.txt".to_owned()]));
    }

    #[test]
    fn opens_files_highlighted_by_their_language() {
        let dir = checkout();
        let reader = FileReader::new(Box::new(LsFiles));
        let file = reader.open(dir.path(), "src/a.rs").expect("opens");
        assert_eq!(file.language.as_deref(), Some("rust"));
        assert_eq!((file.lines.len(), file.truncated), (1, false));
        assert_eq!(file.lines[0].spans[0].token, SyntaxToken::Keyword);
    }

    #[test]
    fn refuses_paths_out_of_the_checkout_binaries_and_missing_files() {
        let dir = checkout();
        let outside = tempfile::tempdir().expect("tempdir");
        std::fs::write(outside.path().join("secret"), "x").expect("write");
        std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("link"))
            .expect("symlink");
        let reader = FileReader::new(Box::new(LsFiles));
        let err = |path| reader.open(dir.path(), path).expect_err(path);
        assert!(err("../etc/passwd").contains("outside the checkout"));
        assert!(err("/etc/passwd").contains("outside the checkout"));
        assert!(err("link").contains("links outside"));
        assert!(err("bin").contains("binary"));
        assert!(err("nope.rs").starts_with("can't read nope.rs"));
        assert!(!FileReader::is_inside(""));
    }

    #[test]
    fn long_files_are_cut() {
        let dir = checkout();
        let long = "x\n".repeat(MAX_LINES + 5);
        std::fs::write(dir.path().join("long.txt"), long).expect("write");
        let reader = FileReader::new(Box::new(LsFiles));
        let file = reader.open(dir.path(), "long.txt").expect("opens");
        assert_eq!((file.lines.len(), file.truncated), (MAX_LINES, true));
        assert_eq!(file.language, None);
    }
}
