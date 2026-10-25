//! One line of the help page.

/// A heading, or some keys and what they do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpLine {
    /// A section title (`Panes`, `Mouse`…).
    Heading(String),
    /// Keys or a gesture, and what it does.
    Entry {
        /// The keys, as drawn (`hjkl ←↓↑→`, `double-click`).
        keys: String,
        /// What they do.
        text: String,
    },
}

impl HelpLine {
    /// An entry.
    pub fn entry(keys: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Entry {
            keys: keys.into(),
            text: text.into(),
        }
    }

    /// Whether `needle` (lower case) appears in it, ignoring case.
    #[must_use]
    pub fn matches(&self, needle: &str) -> bool {
        let found = |s: &str| s.to_lowercase().contains(needle);
        match self {
            Self::Heading(title) => found(title),
            Self::Entry { keys, text } => found(keys) || found(text),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_ignores_case_and_looks_at_keys_and_text() {
        let line = HelpLine::entry("Tab", "jump to the next agent");
        assert!(line.matches("tab") && line.matches("agent"));
        assert!(!line.matches("zoom"));
        assert!(HelpLine::Heading("Mouse".into()).matches("mou"));
    }
}
