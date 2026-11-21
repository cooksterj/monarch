//! Highlights source text: tree-sitter's events become lines of runs,
//! each tagged with the kind of code it is.

mod line_builder;

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use monarch_core::SyntaxToken;
use monarch_proto::CodeLine;
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent};

use crate::{Language, SyntaxError};
use line_builder::LineBuilder;

/// The capture names Monarch colours and the kind each means; a query's
/// capture takes the entry sharing the most of its dotted parts
/// (`function.method` is a function, `string.special.key` a property).
/// Captures not listed (operators, punctuation, variables) stay plain.
const CAPTURES: [(&str, SyntaxToken); 18] = [
    ("keyword", SyntaxToken::Keyword),
    ("variable.builtin", SyntaxToken::Keyword),
    ("string", SyntaxToken::String),
    ("escape", SyntaxToken::String),
    ("string.special.key", SyntaxToken::Property),
    ("comment", SyntaxToken::Comment),
    ("function", SyntaxToken::Function),
    ("constructor", SyntaxToken::Function),
    ("type", SyntaxToken::Type),
    ("attribute", SyntaxToken::Type),
    ("tag", SyntaxToken::Type),
    ("constant", SyntaxToken::Constant),
    ("number", SyntaxToken::Constant),
    ("boolean", SyntaxToken::Constant),
    ("property", SyntaxToken::Property),
    ("label", SyntaxToken::Property),
    ("embedded", SyntaxToken::Plain),
    ("punctuation", SyntaxToken::Plain),
];

/// Highlights files, keeping each language's compiled query for the
/// next file in it. Not shared between threads: callers lock it.
pub struct Highlighter {
    engine: tree_sitter_highlight::Highlighter,
    configs: HashMap<Language, HighlightConfiguration>,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self {
            engine: tree_sitter_highlight::Highlighter::new(),
            configs: HashMap::new(),
        }
    }
}

impl Highlighter {
    /// `source` as lines of runs in `language`, or plain when there is no
    /// language or highlighting fails (the file still shows). Control
    /// characters other than tabs become `�`, so a file can't drive the
    /// terminal it is drawn on.
    pub fn highlight(&mut self, language: Option<Language>, source: &str) -> Vec<CodeLine> {
        language
            .and_then(|lang| self.try_highlight(lang, source).ok())
            .unwrap_or_else(|| LineBuilder::plain(source))
    }

    /// `source` highlighted as `language`.
    ///
    /// # Errors
    /// The language's query doesn't compile, or tree-sitter failed.
    pub fn try_highlight(
        &mut self,
        language: Language,
        source: &str,
    ) -> Result<Vec<CodeLine>, SyntaxError> {
        let config = match self.configs.entry(language) {
            Entry::Occupied(found) => found.into_mut(),
            Entry::Vacant(slot) => slot.insert(language.configuration(&CAPTURES.map(|c| c.0))?),
        };
        let failed = |e: tree_sitter_highlight::Error| SyntaxError::Highlight(e.to_string());
        let events = self
            .engine
            .highlight(config, source.as_bytes(), None, |_| None)
            .map_err(failed)?;
        let mut out = LineBuilder::default();
        let mut kinds = Vec::new();
        for event in events {
            match event.map_err(failed)? {
                HighlightEvent::HighlightStart(Highlight(i)) => {
                    kinds.push(CAPTURES.get(i).map_or(SyntaxToken::Plain, |c| c.1));
                }
                HighlightEvent::HighlightEnd => {
                    kinds.pop();
                }
                HighlightEvent::Source { start, end } => {
                    let kind = kinds.last().copied().unwrap_or_default();
                    out.push(kind, source.get(start..end).unwrap_or_default());
                }
            }
        }
        Ok(out.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(lines: &[CodeLine]) -> Vec<Vec<(SyntaxToken, &str)>> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| (s.token, s.text.as_str())).collect())
            .collect()
    }

    #[test]
    fn rust_comes_back_as_lines_of_kinds() {
        let mut hl = Highlighter::default();
        let lines = hl.highlight(Some(Language::Rust), "// hi\nfn main() { let x = 1; }\n");
        assert_eq!(
            lines.len(),
            2,
            "the final newline ends a line, not starts one"
        );
        let got = kinds(&lines);
        assert_eq!(got[0], [(SyntaxToken::Comment, "// hi")]);
        assert!(
            got[1].contains(&(SyntaxToken::Keyword, "fn")),
            "{:?}",
            got[1]
        );
        assert!(
            got[1].contains(&(SyntaxToken::Function, "main")),
            "{:?}",
            got[1]
        );
        assert!(
            got[1].contains(&(SyntaxToken::Constant, "1")),
            "{:?}",
            got[1]
        );
        assert_eq!(lines[1].text(), "fn main() { let x = 1; }");
    }

    #[test]
    fn every_language_highlights_something_and_keeps_the_text() {
        let samples = [
            (Language::Bash, "echo \"hi\" # note"),
            (Language::C, "int main(void) { return 0; }"),
            (Language::Go, "func main() { fmt.Println(\"x\") }"),
            (Language::JavaScript, "const a = <div>{1}</div>;"),
            (Language::Json, "{\"key\": true}"),
            (Language::Python, "def f():\n    return \"s\""),
            (Language::Rust, "pub struct A;"),
            (Language::Toml, "[package]\nname = \"x\""),
            (Language::TypeScript, "let a: number = 1;"),
            (Language::Tsx, "const A = (): JSX.Element => <p/>;"),
            (Language::Yaml, "key: [1, true]"),
        ];
        let mut hl = Highlighter::default();
        for (lang, source) in samples {
            let lines = hl.try_highlight(lang, source);
            let lines = lines.unwrap_or_else(|e| panic!("{}: {e}", lang.name()));
            let text: Vec<String> = lines.iter().map(CodeLine::text).collect();
            assert_eq!(text.join("\n"), source, "{}", lang.name());
            let coloured = lines
                .iter()
                .flat_map(|l| &l.spans)
                .any(|s| s.token != SyntaxToken::Plain);
            assert!(coloured, "{}: nothing highlighted", lang.name());
        }
    }

    #[test]
    fn unknown_languages_and_control_characters_come_back_plain() {
        let mut hl = Highlighter::default();
        let lines = hl.highlight(None, "a\x1b[31mb\r\n\tc");
        assert_eq!(
            kinds(&lines),
            [
                vec![(SyntaxToken::Plain, "a�[31mb")],
                vec![(SyntaxToken::Plain, "\tc")]
            ]
        );
        assert!(hl.highlight(Some(Language::Rust), "").is_empty());
    }
}
