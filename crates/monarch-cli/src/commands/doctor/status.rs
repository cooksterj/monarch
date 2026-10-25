//! How one doctor finding came out.

/// A finding's outcome, from fine to broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Status {
    /// Working.
    Ok,
    /// Worth knowing; nothing to fix.
    Note,
    /// Works, but not as well as it could.
    Warn,
    /// Broken: something Monarch relies on is missing.
    Fail,
}

impl Status {
    /// The mark at the start of its line.
    #[must_use]
    pub fn mark(self) -> &'static str {
        match self {
            Self::Ok => "✓",
            Self::Note => "·",
            Self::Warn => "!",
            Self::Fail => "✗",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_differ_and_failures_rank_worst() {
        assert_eq!(Status::Fail.mark(), "✗");
        assert_ne!(Status::Warn.mark(), Status::Note.mark());
        assert!(Status::Fail > Status::Warn && Status::Warn > Status::Ok);
    }
}
