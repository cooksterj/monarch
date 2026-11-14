//! One task an agent still has in flight when its turn ends.

use serde::{Deserialize, Serialize};

/// An entry of Claude Code's `background_tasks` on `Stop`: a background
/// shell, sub-agent, teammate, monitor and so on.
///
/// Its id is not always the agent's: a background sub-agent's task id is
/// its `agent_id`, a teammate's is not (`tj1z2y49j` for `atimer-1-…`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTask {
    /// The task's id.
    pub id: String,
    /// Its kind as the agent labels it (`shell`, `subagent`, `teammate`).
    #[serde(default)]
    pub kind: String,
}

impl BackgroundTask {
    /// Whether it is an agent of some kind (a sub-agent or a teammate)
    /// rather than a shell, monitor, workflow or tool call.
    #[must_use]
    pub fn is_agent(&self) -> bool {
        self.kind == "teammate" || self.kind.contains("agent")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(kind: &str) -> BackgroundTask {
        BackgroundTask {
            id: "t1".into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn agents_are_told_from_other_work() {
        for kind in ["subagent", "teammate", "local_agent", "remote_agent"] {
            assert!(task(kind).is_agent(), "{kind}");
        }
        for kind in [
            "shell",
            "monitor",
            "workflow",
            "MCP task",
            "cloud session",
            "",
        ] {
            assert!(!task(kind).is_agent(), "{kind}");
        }
    }

    #[test]
    fn a_task_without_a_kind_reads_as_none() {
        let read: BackgroundTask = serde_json::from_str(r#"{"id":"t1"}"#).expect("json");
        assert_eq!(read, task(""));
        assert!(serde_json::from_str::<BackgroundTask>(r#"{"kind":"shell"}"#).is_err());
    }
}
