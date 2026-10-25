//! What a line of a diff is.

use serde::{Deserialize, Serialize};

/// A diff line's role, which decides its colour and prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DiffLineKind {
    /// `@@ -1,3 +1,4 @@`: where a hunk starts.
    Hunk,
    /// Unchanged, shown for context.
    Context,
    /// Added.
    Added,
    /// Removed.
    Removed,
    /// A note, such as `Binary file` or `No newline at end of file`.
    Note,
}

impl DiffLineKind {
    /// The marker a unified diff puts before the line's text.
    #[must_use]
    pub fn marker(self) -> &'static str {
        match self {
            Self::Added => "+",
            Self::Removed => "-",
            Self::Context => " ",
            Self::Hunk | Self::Note => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_follow_unified_diffs() {
        assert_eq!(DiffLineKind::Added.marker(), "+");
        assert_eq!(DiffLineKind::Removed.marker(), "-");
        assert_eq!(DiffLineKind::Hunk.marker(), "");
    }
}
