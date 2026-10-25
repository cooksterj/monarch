//! What an agent in a pane is doing, as clients show it.

use monarch_core::AgentState;
use serde::{Deserialize, Serialize};

/// A pane's agent state and its one-line activity (`Edit src/main.rs`,
/// the first line of the last answer). Plain shells are `Unknown` with no
/// activity.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentStatus {
    /// The fused state.
    pub state: AgentState,
    /// What it is doing or last said, one line.
    pub activity: Option<String>,
}

impl AgentStatus {
    /// Longest activity line kept, in characters.
    pub const MAX_ACTIVITY: usize = 200;

    /// `text` as an activity line: its first non-blank line, trimmed and
    /// cut to [`Self::MAX_ACTIVITY`]; `None` when there is nothing left.
    #[must_use]
    pub fn activity_line(text: &str) -> Option<String> {
        let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
        Some(line.chars().take(Self::MAX_ACTIVITY).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_shell_is_unknown_and_quiet() {
        let status = AgentStatus::default();
        assert_eq!(status.state, AgentState::Unknown);
        assert_eq!(status.activity, None);
        let bytes = postcard::to_allocvec(&status).expect("encode");
        assert_eq!(
            postcard::from_bytes::<AgentStatus>(&bytes).ok(),
            Some(status)
        );
    }

    #[test]
    fn activity_is_the_first_real_line_cut_short() {
        assert_eq!(
            AgentStatus::activity_line("\n  \n  Added JWT  \nmore").as_deref(),
            Some("Added JWT")
        );
        assert_eq!(AgentStatus::activity_line(" \n\t"), None);
        let long = "x".repeat(500);
        let cut = AgentStatus::activity_line(&long).expect("line");
        assert_eq!(cut.chars().count(), AgentStatus::MAX_ACTIVITY);
    }
}
