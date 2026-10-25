//! One line of a file's diff.

use serde::{Deserialize, Serialize};

use super::DiffLineKind;

/// A diff line: its role and its text, without the `+`/`-`/` ` marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    /// What the line is.
    pub kind: DiffLineKind,
    /// Its text.
    pub text: String,
}

impl DiffLine {
    /// A `kind` line reading `text`.
    #[must_use]
    pub fn new(kind: DiffLineKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_keep_kind_and_text() {
        let line = DiffLine::new(DiffLineKind::Added, "fn main() {}");
        assert_eq!(line.kind, DiffLineKind::Added);
        assert_ne!(line, DiffLine::new(DiffLineKind::Removed, "fn main() {}"));
    }
}
