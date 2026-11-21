//! Syntax highlighting for Monarch: source text in, lines of runs tagged
//! with the kind of code they are ([`monarch_core::SyntaxToken`]) out,
//! using tree-sitter's grammars and highlight queries. No IO and no
//! colours: the server reads files, the client's theme colours tokens.

mod error;
mod highlighter;
mod language;

pub use error::SyntaxError;
pub use highlighter::Highlighter;
pub use language::Language;
