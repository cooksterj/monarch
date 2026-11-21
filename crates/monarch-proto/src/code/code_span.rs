//! A run of one line of source that is all one kind of code.

use monarch_core::SyntaxToken;
use serde::{Deserialize, Serialize};

/// Text and the kind of code it is; never holds a newline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeSpan {
    /// What kind of code it is.
    pub token: SyntaxToken,
    /// The text, tabs kept.
    pub text: String,
}

impl CodeSpan {
    /// A span of `text` of kind `token`.
    #[must_use]
    pub fn new(token: SyntaxToken, text: impl Into<String>) -> Self {
        Self {
            token,
            text: text.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_compare_by_kind_and_text() {
        let span = CodeSpan::new(SyntaxToken::String, "\"hi\"");
        assert_eq!(span, CodeSpan::new(SyntaxToken::String, "\"hi\""));
        assert_ne!(span, CodeSpan::new(SyntaxToken::Plain, "\"hi\""));
        assert_eq!(CodeSpan::default().token, SyntaxToken::Plain);
    }
}
