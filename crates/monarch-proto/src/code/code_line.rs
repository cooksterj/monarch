//! One line of a highlighted file.

use serde::{Deserialize, Serialize};

use super::CodeSpan;

/// A line's spans, left to right; empty for a blank line.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeLine {
    /// The runs of text, each one kind of code.
    pub spans: Vec<CodeSpan>,
}

impl CodeLine {
    /// The line as plain text.
    #[must_use]
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::SyntaxToken;

    use super::*;

    #[test]
    fn text_joins_the_spans() {
        let line = CodeLine {
            spans: vec![
                CodeSpan::new(SyntaxToken::Keyword, "let"),
                CodeSpan::new(SyntaxToken::Plain, " x = "),
                CodeSpan::new(SyntaxToken::Constant, "1"),
            ],
        };
        assert_eq!(line.text(), "let x = 1");
        assert_eq!(CodeLine::default().text(), "");
    }
}
