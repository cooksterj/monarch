//! Each language's tree-sitter grammar and highlight query.

use tree_sitter_highlight::HighlightConfiguration;

use super::Language;
use crate::SyntaxError;

impl Language {
    /// Its grammar with its highlight query compiled, recognising
    /// `captures`. Compiling takes milliseconds: callers keep the result.
    ///
    /// # Errors
    /// The query doesn't compile against the grammar (a packaging bug).
    pub(crate) fn configuration(
        self,
        captures: &[&str],
    ) -> Result<HighlightConfiguration, SyntaxError> {
        let query = self.query();
        let mut config = HighlightConfiguration::new(self.grammar(), self.name(), &query, "", "")
            .map_err(|e| SyntaxError::Query {
            language: self.name(),
            message: e.to_string(),
        })?;
        config.configure(captures);
        Ok(config)
    }

    /// Its tree-sitter grammar.
    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::Bash => tree_sitter_bash::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Json => tree_sitter_json::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Toml => tree_sitter_toml_ng::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::Yaml => tree_sitter_yaml::LANGUAGE.into(),
        }
    }

    /// Its highlight query. TypeScript's only adds to JavaScript's, so it
    /// is laid over it, as editors do; JSX patterns join where JSX parses.
    fn query(self) -> String {
        let js = tree_sitter_javascript::HIGHLIGHT_QUERY;
        let jsx = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;
        let ts = tree_sitter_typescript::HIGHLIGHTS_QUERY;
        match self {
            Self::Bash => tree_sitter_bash::HIGHLIGHT_QUERY.into(),
            Self::C => tree_sitter_c::HIGHLIGHT_QUERY.into(),
            Self::Go => tree_sitter_go::HIGHLIGHTS_QUERY.into(),
            Self::JavaScript => [js, jsx].concat(),
            Self::Json => tree_sitter_json::HIGHLIGHTS_QUERY.into(),
            Self::Python => tree_sitter_python::HIGHLIGHTS_QUERY.into(),
            Self::Rust => tree_sitter_rust::HIGHLIGHTS_QUERY.into(),
            Self::Toml => tree_sitter_toml_ng::HIGHLIGHTS_QUERY.into(),
            Self::TypeScript => [ts, js].concat(),
            Self::Tsx => [ts, js, jsx].concat(),
            Self::Yaml => tree_sitter_yaml::HIGHLIGHTS_QUERY.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_query_compiles_against_its_grammar() {
        for lang in Language::ALL {
            let config = lang.configuration(&["keyword"]);
            assert!(config.is_ok(), "{}: {:?}", lang.name(), config.err());
        }
    }
}
