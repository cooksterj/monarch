//! Cuts highlighted runs into lines, merging neighbouring runs of the
//! same kind and making control characters harmless.

use monarch_core::SyntaxToken;
use monarch_proto::{CodeLine, CodeSpan};

/// The lines so far and the one being built.
#[derive(Debug, Default)]
pub(super) struct LineBuilder {
    lines: Vec<CodeLine>,
    current: CodeLine,
}

impl LineBuilder {
    /// `source` as plain lines.
    pub(super) fn plain(source: &str) -> Vec<CodeLine> {
        let mut out = Self::default();
        out.push(SyntaxToken::Plain, source);
        out.finish()
    }

    /// Adds `text` of kind `kind`; each newline in it ends a line, and a
    /// carriage return before one is dropped.
    pub(super) fn push(&mut self, kind: SyntaxToken, text: &str) {
        for (i, piece) in text.split('\n').enumerate() {
            if i > 0 {
                let line = std::mem::take(&mut self.current);
                self.lines.push(strip_cr(line));
            }
            if piece.is_empty() {
                continue;
            }
            let clean: String = piece.chars().map(harmless).collect();
            match self.current.spans.last_mut() {
                Some(last) if last.token == kind => last.text.push_str(&clean),
                _ => self.current.spans.push(CodeSpan::new(kind, clean)),
            }
        }
    }

    /// The lines; a final newline ends the last line rather than starting
    /// an empty one.
    pub(super) fn finish(mut self) -> Vec<CodeLine> {
        if !self.current.spans.is_empty() {
            self.lines.push(self.current);
        }
        self.lines
    }
}

/// A control character other than a tab, drawn as `�` instead of sent to
/// the terminal; a carriage return is kept until [`strip_cr`] sees
/// whether it ends the line.
fn harmless(c: char) -> char {
    if c.is_control() && c != '\t' && c != '\r' {
        '\u{fffd}'
    } else {
        c
    }
}

/// Drops a carriage return ending `line` (CRLF files); any other becomes
/// `�`.
fn strip_cr(mut line: CodeLine) -> CodeLine {
    if let Some(last) = line.spans.last_mut()
        && last.text.ends_with('\r')
    {
        last.text.pop();
        if last.text.is_empty() {
            line.spans.pop();
        }
    }
    for span in &mut line.spans {
        if span.text.contains('\r') {
            span.text = span.text.replace('\r', "\u{fffd}");
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_split_at_newlines_and_merge_by_kind() {
        let mut b = LineBuilder::default();
        b.push(SyntaxToken::Keyword, "let");
        b.push(SyntaxToken::Keyword, " mut");
        b.push(SyntaxToken::String, "\"a\nb\"");
        let lines = b.finish();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0].spans,
            [
                CodeSpan::new(SyntaxToken::Keyword, "let mut"),
                CodeSpan::new(SyntaxToken::String, "\"a")
            ]
        );
        assert_eq!(lines[1].spans, [CodeSpan::new(SyntaxToken::String, "b\"")]);
    }

    #[test]
    fn blank_lines_stay_and_crlf_is_undone() {
        let lines = LineBuilder::plain("a\r\n\r\n\nb\rc\n");
        let text: Vec<String> = lines.iter().map(CodeLine::text).collect();
        assert_eq!(text, ["a", "", "", "b\u{fffd}c"]);
        assert!(LineBuilder::plain("").is_empty());
    }
}
