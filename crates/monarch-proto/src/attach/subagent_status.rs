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
    /// What its task is called (`Audit the auth module`), when the
    /// parent's Agent tool said.
    pub label: Option<String>,
    /// Working, Blocked or Done.
    pub state: AgentState,
    /// What it is doing or last said, one line.
    pub activity: Option<String>,
}

impl SubagentStatus {
    /// What it is called where it is shown: its task, else its kind,
    /// else `subagent`.
    #[must_use]
    pub fn label(&self) -> &str {
        match self.label.as_deref().filter(|l| !l.is_empty()) {
            Some(label) => label,
            None if self.kind.is_empty() => "subagent",
            None => &self.kind,
        }
    }

    /// Its parent's activity line while it waits on the user: the
    /// question first, since a narrow sidebar keeps only the start
    /// (`Allow Bash cargo test? · Explore`).
    #[must_use]
    pub fn asking(&self) -> Option<String> {
        let line = match &self.activity {
            Some(activity) => format!("{activity} · {}", self.label()),
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
            label: None,
            state: AgentState::Blocked,
            activity: activity.map(Into::into),
        }
    }

    #[test]
    fn it_is_named_by_its_task_its_kind_or_generically() {
        assert_eq!(sub("Explore", None).label(), "Explore");
        assert_eq!(sub("", None).label(), "subagent");
        let named = SubagentStatus {
            label: Some("Audit auth".into()),
            ..sub("Explore", None)
        };
        assert_eq!(named.label(), "Audit auth");
        let blank = SubagentStatus {
            label: Some(String::new()),
            ..sub("Explore", None)
        };
        assert_eq!(blank.label(), "Explore", "an empty task name is no name");
    }

    #[test]
    fn its_question_names_it_and_is_one_short_line() {
        assert_eq!(
            sub("Plan", Some("Allow Bash ls?")).asking().as_deref(),
            Some("Allow Bash ls? · Plan")
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
