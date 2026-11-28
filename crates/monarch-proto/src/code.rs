//! A file from a checkout, highlighted for the file viewer: its lines as
//! runs of source text, each with the kind of code it is.

mod code_line;
mod code_span;
mod quote_source;

use serde::{Deserialize, Serialize};

pub use code_line::CodeLine;
pub use code_span::CodeSpan;
pub use quote_source::QuoteSource;

/// One file, read and highlighted by the server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeFile {
    /// The file, from the checkout's top.
    pub path: String,
    /// The language it was highlighted as (`rust`); `None` for plain text.
    pub language: Option<String>,
    /// Its lines, first to last.
    pub lines: Vec<CodeLine>,
    /// The file was longer than the server sends; the rest is left out.
    pub truncated: bool,
}

impl CodeFile {
    /// Lines `start..=end` (counting from 1, clamped to the file) as plain
    /// text, one string per line.
    #[must_use]
    pub fn text(&self, start: usize, end: usize) -> Vec<String> {
        let from = start.max(1) - 1;
        let to = end.min(self.lines.len());
        self.lines
            .get(from..to)
            .unwrap_or_default()
            .iter()
            .map(CodeLine::text)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::SyntaxToken;

    use super::*;

    fn file() -> CodeFile {
        let line = |text: &str| CodeLine {
            spans: vec![CodeSpan::new(SyntaxToken::Keyword, text)],
        };
        CodeFile {
            path: "src/a.rs".into(),
            language: Some("rust".into()),
            lines: vec![line("fn a"), line("fn b"), line("fn c")],
            truncated: false,
        }
    }

    #[test]
    fn lines_are_counted_from_one_and_clamped() {
        let file = file();
        assert_eq!(file.text(2, 3), ["fn b", "fn c"]);
        assert_eq!(file.text(0, 1), ["fn a"]);
        assert_eq!(file.text(3, 99), ["fn c"]);
        assert!(file.text(5, 9).is_empty(), "past the end");
        assert!(file.text(3, 2).is_empty(), "backwards");
    }

    #[test]
    fn files_survive_the_attach_encoding() {
        let file = file();
        let bytes = postcard::to_stdvec(&file).expect("encode");
        assert_eq!(postcard::from_bytes::<CodeFile>(&bytes).ok(), Some(file));
    }
}
