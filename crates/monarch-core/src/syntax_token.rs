//! What a piece of source code is, as far as colouring it goes: the
//! highlighter's output and the theme's input.

use serde::{Deserialize, Serialize};

/// The kind of a run of source text. Each kind but `Plain` has a theme
/// colour; `Plain` is drawn in the pop-up's text colour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SyntaxToken {
    /// Identifiers, operators and punctuation.
    #[default]
    Plain,
    /// `fn`, `if`, `return`, `self`.
    Keyword,
    /// String and character literals, escapes included.
    String,
    /// Comments, doc comments included.
    Comment,
    /// Function, method and macro names, and constructors.
    Function,
    /// Type names, attributes and markup tags.
    Type,
    /// Numbers, booleans and named constants.
    Constant,
    /// Fields, keys in data files (JSON, TOML, YAML) and labels.
    Property,
}

impl SyntaxToken {
    /// Every kind, `Plain` first.
    pub const ALL: [Self; 8] = [
        Self::Plain,
        Self::Keyword,
        Self::String,
        Self::Comment,
        Self::Function,
        Self::Type,
        Self::Constant,
        Self::Property,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_is_the_default_and_every_kind_is_listed_once() {
        assert_eq!(SyntaxToken::default(), SyntaxToken::Plain);
        let unique: std::collections::HashSet<_> = SyntaxToken::ALL.into_iter().collect();
        assert_eq!(unique.len(), SyntaxToken::ALL.len());
    }
}
