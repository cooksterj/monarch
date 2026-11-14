//! What one sub-agent inside a pane's agent is doing.

use monarch_core::AgentState;
use serde::{Deserialize, Serialize};

use crate::AgentStatus;

/// A sub-agent of a pane's agent, as its hooks reported it. It has no
/// pane of its own: whatever it asks is answered in its parent's pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentStatus {
    /// The agent's id for it.
    pub id: String,
    /// What kind it is (`Explore`); empty when its hooks never said.
    pub kind: String,
    /// Working, Blocked or Done.
    pub state: AgentState,
    /// What it is doing or last said, one line.
    pub activity: Option<String>,
}

impl SubagentStatus {
    /// What it is called where it is shown: its kind, else `subagent`.
    #[must_use]
    pub fn label(&self) -> &str {
        if self.kind.is_empty() {
            "subagent"
        } else {
            &self.kind
        }
    }

    /// Its parent's activity line while it waits on the user:
    /// `Explore: Allow Bash cargo test?`.
    #[must_use]
    pub fn asking(&self) -> Option<String> {
        let line = match &self.activity {
            Some(activity) => format!("{}: {activity}", self.label()),
            None => format!("{} needs you", self.label()),
        };
        AgentStatus::activity_line(&line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(kind: &str, activity: Option<&str>) -> SubagentStatus {
        SubagentStatus {
            id: "a1".into(),
            kind: kind.into(),
            state: AgentState::Blocked,
            activity: activity.map(Into::into),
        }
    }

    #[test]
    fn it_is_named_by_its_kind_or_generically() {
        assert_eq!(sub("Explore", None).label(), "Explore");
        assert_eq!(sub("", None).label(), "subagent");
    }

    #[test]
    fn its_question_names_it_and_is_one_short_line() {
        assert_eq!(
            sub("Plan", Some("Allow Bash ls?")).asking().as_deref(),
            Some("Plan: Allow Bash ls?")
        );
        assert_eq!(
            sub("", None).asking().as_deref(),
            Some("subagent needs you")
        );
        let long = "x".repeat(500);
        let cut = sub("Plan", Some(&long)).asking().expect("line");
        assert_eq!(cut.chars().count(), AgentStatus::MAX_ACTIVITY);
        let bytes = postcard::to_allocvec(&sub("Plan", None)).expect("encode");
        assert_eq!(
            postcard::from_bytes::<SubagentStatus>(&bytes).ok(),
            Some(sub("Plan", None))
        );
    }
}
