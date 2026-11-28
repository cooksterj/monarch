//! Finds where text selected on a pane's screen lives in its checkout:
//! `git grep` for its most telling line, then each hit checked against
//! every selected line, so an ask can name `path:lines`.

use std::collections::HashMap;
use std::path::Path;

use monarch_proto::QuoteSource;
use monarch_syntax::Language;

use super::{FileReader, confined, read_text};

/// Shortest line worth searching for; shorter ones match everywhere.
const MIN_NEEDLE: usize = 6;
/// Most hits checked; a line found more often names no one place.
const MAX_HITS: usize = 200;

impl FileReader {
    /// The one place in the checkout at `root` whose lines `text` (as
    /// shown on screen: maybe behind a line-number or diff gutter, maybe
    /// starting or ending mid-line) shows; `None` when it is nowhere, in
    /// more than one place, or too plain to tell. Runs git and reads the
    /// disk: call it off the async runtime.
    #[must_use]
    pub fn locate(&self, root: &Path, text: &str) -> Option<QuoteSource> {
        let shown = Shown::new(text)?;
        let mut args = vec!["grep", "-z", "-n", "-I", "-F", "--untracked", "--no-color"];
        args.extend(["-e", shown.needle.as_str(), "--"]);
        let out = self.git.run(root, &args)?;
        let hits: Vec<(&str, usize)> = out.lines().filter_map(hit).collect();
        if hits.len() > MAX_HITS {
            return None;
        }
        let mut files: HashMap<&str, Option<Vec<String>>> = HashMap::new();
        let mut found: Option<(&str, usize)> = None;
        for (path, number) in hits {
            let Some(start) = number.checked_sub(shown.anchor).filter(|s| *s >= 1) else {
                continue;
            };
            let lines = files.entry(path).or_insert_with(|| lines_of(root, path));
            if lines.as_ref().is_some_and(|l| shown.is_at(l, start)) {
                if found.is_some_and(|f| f != (path, start)) {
                    return None;
                }
                found = Some((path, start));
            }
        }
        let (path, start) = found?;
        let lines = files.get(path)?.as_ref()?;
        Some(QuoteSource {
            path: path.to_owned(),
            language: Language::for_path(Path::new(path)).map(|l| l.name().to_owned()),
            start,
            lines: lines
                .get(start - 1..start - 1 + shown.lines.len())?
                .to_vec(),
        })
    }
}

/// A selection's lines, blank edges dropped, and the line searched for.
struct Shown<'a> {
    /// The lines, trimmed.
    lines: Vec<&'a str>,
    /// Which line the needle is from.
    anchor: usize,
    /// The longest line's code, gutter dropped.
    needle: String,
}

impl<'a> Shown<'a> {
    fn new(text: &'a str) -> Option<Self> {
        let all: Vec<&str> = text.lines().map(str::trim).collect();
        let first = all.iter().position(|l| !is_gutter(l))?;
        let last = all.iter().rposition(|l| !is_gutter(l))?;
        let lines = all[first..=last].to_vec();
        let (anchor, needle) = lines
            .iter()
            .map(|l| l.trim_start_matches(is_gutter_char))
            .enumerate()
            .max_by_key(|(i, code)| (code.chars().count(), usize::MAX - i))?;
        let needle = needle.to_owned();
        (needle.chars().count() >= MIN_NEEDLE).then_some(Self {
            lines,
            anchor,
            needle,
        })
    }

    /// Whether the file's `lines` from line `start` (from 1) are these.
    fn is_at(&self, lines: &[String], start: usize) -> bool {
        let last = self.lines.len() - 1;
        self.lines.iter().enumerate().all(|(i, shown)| {
            lines
                .get(start - 1 + i)
                .is_some_and(|line| matches(shown, line.trim(), i == 0, i == last))
        })
    }
}

/// Whether `shown` (trimmed) is the file line `line` (trimmed): the whole
/// line behind at most a gutter, or, at a selection's first or last line
/// of several, the part of it the selection began or ended in.
fn matches(shown: &str, line: &str, first: bool, last: bool) -> bool {
    if line.is_empty() {
        return is_gutter(shown);
    }
    let whole = shown.strip_suffix(line).is_some_and(is_gutter);
    let several = !(first && last);
    whole
        || (several && first && !shown.is_empty() && line.ends_with(shown))
        || (several && last && !shown.is_empty() && line.starts_with(shown))
}

/// What may stand before a line on screen and isn't code: line numbers,
/// rules and diff markers.
fn is_gutter_char(c: char) -> bool {
    c.is_whitespace() || c.is_ascii_digit() || matches!(c, '│' | '┃' | '|' | ':' | '+' | '-')
}

fn is_gutter(text: &str) -> bool {
    text.chars().all(is_gutter_char)
}

/// A `git grep -z -n` line: the path and the line number.
fn hit(line: &str) -> Option<(&str, usize)> {
    let mut parts = line.splitn(3, '\0');
    let path = parts.next()?;
    let number = parts.next()?.parse().ok()?;
    Some((path, number))
}

/// The lines of `path` in the checkout at `root`, if it reads as text.
fn lines_of(root: &Path, path: &str) -> Option<Vec<String>> {
    let real = confined(root, path).ok()?;
    let (text, _) = read_text(&real, path).ok()?;
    Some(text.lines().map(str::to_owned).collect())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::GitRunner;

    /// Answers `git grep` as git would for the files written, by plain
    /// substring.
    struct Grep;

    impl GitRunner for Grep {
        fn run(&self, dir: &Path, args: &[&str]) -> Option<String> {
            let needle = args[args.len() - 2];
            let mut out = Vec::new();
            for path in ["src/a.rs", "src/b.rs", "notes.md"] {
                let text = std::fs::read_to_string(dir.join(path)).ok()?;
                for (i, line) in text.lines().enumerate() {
                    if line.contains(needle) {
                        out.push(format!("{path}\0{}\0{line}", i + 1));
                    }
                }
            }
            (!out.is_empty()).then(|| out.join("\n"))
        }
    }

    const A: &str =
        "fn main() {\n    let total = compute(1, 2);\n\n    println!(\"{total}\");\n}\n";
    const B: &str = "fn other() {\n    let total = compute(1, 2);\n    done();\n}\n";

    fn checkout() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("src")).expect("mkdir");
        std::fs::write(dir.path().join("src/a.rs"), A).expect("write");
        std::fs::write(dir.path().join("src/b.rs"), B).expect("write");
        std::fs::write(dir.path().join("notes.md"), "plain words here\n").expect("write");
        dir
    }

    fn locate(text: &str) -> Option<(String, usize, Vec<String>)> {
        let dir = checkout();
        let reader = FileReader::new(Box::new(Grep));
        let found = reader.locate(dir.path(), text)?;
        Some((found.path, found.start, found.lines))
    }

    #[test]
    fn a_selection_is_found_with_its_lines_from_the_file() {
        let found = locate("  let total = compute(1, 2);\n\n  println!(\"{total}\");\n\n");
        let lines = vec![
            "    let total = compute(1, 2);".to_owned(),
            String::new(),
            "    println!(\"{total}\");".to_owned(),
        ];
        assert_eq!(found, Some(("src/a.rs".into(), 2, lines)));
        let dir = checkout();
        let reader = FileReader::new(Box::new(Grep));
        let source = reader
            .locate(dir.path(), "plain words here")
            .expect("found");
        assert_eq!((source.path.as_str(), source.language), ("notes.md", None));
        let rust = reader.locate(dir.path(), "fn main() {").expect("found");
        assert_eq!(rust.language.as_deref(), Some("rust"));
    }

    #[test]
    fn gutters_and_lines_cut_at_the_edges_still_match() {
        let numbered = "  1 │ fn main() {\n  2 │     let total = compute(1, 2);";
        assert_eq!(locate(numbered).map(|f| f.1), Some(1));
        let cut = "main() {\n    let total = compute(1, 2);\n\n    print";
        assert_eq!(
            locate(cut).map(|f| (f.0, f.1)),
            Some(("src/a.rs".into(), 1))
        );
        let diff = "+    let total = compute(1, 2);\n+    done();";
        assert_eq!(
            locate(diff).map(|f| (f.0, f.1)),
            Some(("src/b.rs".into(), 2))
        );
    }

    #[test]
    fn ambiguous_plain_or_absent_text_names_no_place() {
        assert_eq!(locate("let total = compute(1, 2);"), None, "in two files");
        assert_eq!(locate("}\n}"), None, "too plain to search");
        assert_eq!(locate("\n \n"), None, "blank");
        assert_eq!(locate("nowhere in the checkout"), None);
        assert_eq!(locate("words here"), None, "one line must be whole");
        let wrong_next = "let total = compute(1, 2);\n    other();";
        assert_eq!(locate(wrong_next), None, "every line is checked");
    }
}
