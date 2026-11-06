//! The answer to `agent.read`.

use serde::{Deserialize, Serialize};

use crate::{AgentStatus, ReadSource};

/// What the pane's agent last said, and how it is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentReadReply {
    /// Its status now; a reader can tell a finished answer from one
    /// still being written.
    pub status: AgentStatus,
    /// The text; empty when `source` is `Nothing`.
    pub text: String,
    /// What `text` is.
    pub source: ReadSource,
}

impl AgentReadReply {
    /// An answer with nothing to read.
    #[must_use]
    pub fn nothing(status: AgentStatus) -> Self {
        Self {
            status,
            text: String::new(),
            source: ReadSource::Nothing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let reply = AgentReadReply {
            status: AgentStatus::default(),
            text: "Fixed it.\n\nTwo tests added.".into(),
            source: ReadSource::Message,
        };
        let json = serde_json::to_string(&reply).expect("json");
        assert_eq!(
            serde_json::from_str::<AgentReadReply>(&json).ok(),
            Some(reply)
        );
        let empty = AgentReadReply::nothing(AgentStatus::default());
        assert_eq!(
            (empty.text.as_str(), empty.source),
            ("", ReadSource::Nothing)
        );
        assert!(serde_json::from_str::<AgentReadReply>(r#"{"text":"x"}"#).is_err());
    }
}
