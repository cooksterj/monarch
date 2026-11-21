//! The languages Monarch highlights, and which one a file is in.

mod grammar;

use std::path::Path;

/// A language with a bundled tree-sitter grammar. Files in any other
/// language are shown as plain text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Bash and other POSIX shells.
    Bash,
    /// C and its headers.
    C,
    /// Go.
    Go,
    /// JavaScript, JSX included.
    JavaScript,
    /// JSON.
    Json,
    /// Python.
    Python,
    /// Rust.
    Rust,
    /// TOML.
    Toml,
    /// TypeScript without JSX.
    TypeScript,
    /// TypeScript with JSX.
    Tsx,
    /// YAML.
    Yaml,
}

impl Language {
    /// Every language.
    pub const ALL: [Self; 11] = [
        Self::Bash,
        Self::C,
        Self::Go,
        Self::JavaScript,
        Self::Json,
        Self::Python,
        Self::Rust,
        Self::Toml,
        Self::TypeScript,
        Self::Tsx,
        Self::Yaml,
    ];

    /// Its name, lower case (`rust`, `tsx`), as code fences tag it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::C => "c",
            Self::Go => "go",
            Self::JavaScript => "javascript",
            Self::Json => "json",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Toml => "toml",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Yaml => "yaml",
        }
    }

    /// The language `path` is written in, by its name or extension.
    #[must_use]
    pub fn for_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if let Some(lang) = by_name(name) {
            return Some(lang);
        }
        by_extension(path.extension()?.to_str()?)
    }
}

/// Files known by their whole name.
fn by_name(name: &str) -> Option<Language> {
    Some(match name {
        "Cargo.lock" | "Pipfile" | "poetry.lock" | "uv.lock" => Language::Toml,
        ".bashrc" | ".bash_profile" | ".profile" | ".zshrc" | ".zprofile" | ".envrc" => {
            Language::Bash
        }
        _ => return None,
    })
}

/// Files known by their extension.
fn by_extension(ext: &str) -> Option<Language> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "sh" | "bash" | "zsh" => Language::Bash,
        "c" | "h" => Language::C,
        "go" => Language::Go,
        "js" | "mjs" | "cjs" | "jsx" => Language::JavaScript,
        "json" | "jsonc" => Language::Json,
        "py" | "pyi" => Language::Python,
        "rs" => Language::Rust,
        "toml" => Language::Toml,
        "ts" | "mts" | "cts" => Language::TypeScript,
        "tsx" => Language::Tsx,
        "yml" | "yaml" => Language::Yaml,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_known_by_extension_or_name() {
        let lang = |p: &str| Language::for_path(Path::new(p));
        assert_eq!(lang("crates/a/src/lib.rs"), Some(Language::Rust));
        assert_eq!(lang("web/App.TSX"), Some(Language::Tsx));
        assert_eq!(lang("Cargo.lock"), Some(Language::Toml));
        assert_eq!(lang("/home/me/.zshrc"), Some(Language::Bash));
        assert_eq!(lang(".github/ci.yml"), Some(Language::Yaml));
        assert_eq!(lang("README.md"), None, "no grammar bundled");
        assert_eq!(lang("Makefile"), None);
        assert_eq!(lang(""), None);
    }

    #[test]
    fn names_are_distinct() {
        let names: std::collections::HashSet<_> = Language::ALL.map(Language::name).into();
        assert_eq!(names.len(), Language::ALL.len());
    }
}
