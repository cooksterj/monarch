//! One line of `monarch doctor`'s report.

use super::Status;

/// What a check found, and how to fix it when there is something to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    /// How it came out.
    pub status: Status,
    /// What was checked (`config`, `Claude Code`…).
    pub topic: String,
    /// What was found.
    pub text: String,
    /// A command or step that fixes it.
    pub fix: Option<String>,
}

impl Finding {
    /// A finding with no fix.
    pub fn new(status: Status, topic: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            status,
            topic: topic.into(),
            text: text.into(),
            fix: None,
        }
    }

    /// The same finding with `fix`.
    #[must_use]
    pub fn fix(self, fix: impl Into<String>) -> Self {
        Self {
            fix: Some(fix.into()),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_attach_to_findings() {
        let found = Finding::new(Status::Fail, "git", "not found");
        assert_eq!(found.fix, None);
        let fixed = found.fix("install git");
        assert_eq!(fixed.fix.as_deref(), Some("install git"));
        assert_eq!(fixed.topic, "git");
    }
}
