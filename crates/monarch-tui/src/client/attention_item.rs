//! One agent that needs the user, as the inbox lists it.

use std::time::Duration;

use monarch_core::PaneId;
use monarch_proto::AgentStatus;

/// A blocked, failed or done agent: where it is, what it is called, what
/// it is doing and how long it has been waiting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionItem<'a> {
    /// The tab its pane is in, counting from 0.
    pub tab: usize,
    /// Its pane.
    pub pane: PaneId,
    /// The pane's name in rows and lists.
    pub name: &'a str,
    /// Its state and activity.
    pub status: &'a AgentStatus,
    /// Its turn's working time, when the server said.
    pub age: Option<Duration>,
    /// How long it has been in that state, when the server said.
    pub waited: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentState;

    use super::*;

    #[test]
    fn items_compare_by_content() {
        let status = AgentStatus {
            state: AgentState::Blocked,
            activity: None,
            agent: None,
        };
        let item = AttentionItem {
            tab: 0,
            pane: PaneId(1),
            name: "codex",
            status: &status,
            age: Some(Duration::from_secs(3)),
            waited: Some(Duration::from_secs(9)),
        };
        let mut other = item.clone();
        assert_eq!(item, other);
        other.age = None;
        assert_ne!(item, other);
    }
}
