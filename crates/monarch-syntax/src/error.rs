//! Errors from highlighting.

use thiserror::Error;

/// Why a file couldn't be highlighted (it can still be shown plain).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyntaxError {
    /// A grammar's highlight query didn't compile against its grammar.
    #[error("the {language} highlight query is invalid: {message}")]
    Query {
        /// The language.
        language: &'static str,
        /// What tree-sitter said.
        message: String,
    },
    /// Highlighting started but failed (cancelled, or a parser error).
    #[error("highlighting failed: {0}")]
    Highlight(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_name_the_language() {
        let err = SyntaxError::Query {
            language: "rust",
            message: "bad node".into(),
        };
        assert_eq!(
            err.to_string(),
            "the rust highlight query is invalid: bad node"
        );
        assert!(SyntaxError::Highlight("x".into()).to_string().contains("x"));
    }
}
