//! Highlights a file's diff as code: each hunk's new side (context and
//! added lines) and old side (context and removed lines) are highlighted
//! as the code they are, so tree-sitter sees whole statements.

use std::path::Path;

use monarch_proto::{DiffLineKind, ReviewFile};
use monarch_syntax::{Highlighter, Language};

/// Fills in the spans of `file`'s lines, when its language is known.
pub(super) fn highlight(syntax: &mut Highlighter, file: &mut ReviewFile) {
    let Some(language) = Language::for_path(Path::new(&file.path)) else {
        return;
    };
    let mut start = 0;
    while start < file.lines.len() {
        let end = (start..file.lines.len())
            .find(|&i| i > start && is_break(file.lines[i].kind))
            .unwrap_or(file.lines.len());
        side(syntax, language, file, start..end, DiffLineKind::Added);
        side(syntax, language, file, start..end, DiffLineKind::Removed);
        start = end;
    }
}

/// Where one run of code ends: a hunk header or a note.
fn is_break(kind: DiffLineKind) -> bool {
    matches!(kind, DiffLineKind::Hunk | DiffLineKind::Note)
}

/// Highlights the context and `changed` lines in `range` together, then
/// gives each `changed` line its spans (context lines take the new side's).
fn side(
    syntax: &mut Highlighter,
    language: Language,
    file: &mut ReviewFile,
    range: std::ops::Range<usize>,
    changed: DiffLineKind,
) {
    let lines: Vec<usize> = range
        .filter(|&i| matches!(file.lines[i].kind, k if k == changed || k == DiffLineKind::Context))
        .collect();
    if !lines.iter().any(|&i| file.lines[i].kind == changed) {
        return;
    }
    let source: Vec<&str> = lines.iter().map(|&i| file.lines[i].text.as_str()).collect();
    let code = syntax.highlight(Some(language), &source.join("\n"));
    if code.len() < lines.len() {
        return; // tree-sitter split the lines differently: leave them plain
    }
    for (&i, code) in lines.iter().zip(code) {
        let line = &mut file.lines[i];
        if line.kind == changed || changed == DiffLineKind::Added {
            line.spans = code.spans;
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::SyntaxToken;
    use monarch_proto::{DiffLine, FileChange};

    use super::*;

    fn file(path: &str, lines: &[(DiffLineKind, &str)]) -> ReviewFile {
        let mut file = ReviewFile::new(path, FileChange::Modified);
        file.lines = lines.iter().map(|(k, t)| DiffLine::new(*k, *t)).collect();
        file
    }

    fn kinds(line: &DiffLine) -> Vec<SyntaxToken> {
        line.spans.iter().map(|s| s.token).collect()
    }

    #[test]
    fn both_sides_are_highlighted_and_keep_their_text() {
        let mut f = file(
            "src/a.rs",
            &[
                (DiffLineKind::Hunk, "@@ -1,2 +1,2 @@"),
                (DiffLineKind::Context, "fn main() {"),
                (DiffLineKind::Removed, "    let x = \"old\";"),
                (DiffLineKind::Added, "    let x = \"new\";"),
                (DiffLineKind::Context, "}"),
            ],
        );
        highlight(&mut Highlighter::default(), &mut f);
        assert!(f.lines[0].spans.is_empty(), "the hunk header stays plain");
        assert!(kinds(&f.lines[1]).contains(&SyntaxToken::Keyword), "`fn`");
        for line in &f.lines[2..4] {
            let text: String = line.spans.iter().map(|s| s.text.as_str()).collect();
            assert_eq!(text, line.text);
            assert!(
                kinds(line).contains(&SyntaxToken::String),
                "{:?}",
                line.spans
            );
        }
    }

    #[test]
    fn unknown_languages_and_notes_stay_plain() {
        let mut f = file("notes.unknownext", &[(DiffLineKind::Added, "fn x() {}")]);
        highlight(&mut Highlighter::default(), &mut f);
        assert!(f.lines[0].spans.is_empty());
        let mut f = file("a.rs", &[(DiffLineKind::Note, "Binary file")]);
        highlight(&mut Highlighter::default(), &mut f);
        assert!(f.lines[0].spans.is_empty());
    }
}
