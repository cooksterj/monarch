//! Where the text `agent.read` returns came from.

use serde::{Deserialize, Serialize};

/// What an `agent.read` answer holds, most trustworthy first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ReadSource {
    /// The agent's final message of its last turn, as its hooks sent it.
    Message,
    /// What the pane shows now; the agent has no hooks.
    Screen,
    /// Nothing: the agent has said nothing yet.
    #[default]
    Nothing,
}

impl ReadSource {
    /// How `monarch agent read` names it.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Message => "its last message",
            Self::Screen => "its screen",
            Self::Nothing => "nothing yet",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_snake_case_and_describes_itself() {
        let json = serde_json::to_string(&ReadSource::Message).expect("json");
        assert_eq!(json, "\"message\"");
        assert_eq!(ReadSource::default().describe(), "nothing yet");
        assert!(serde_json::from_str::<ReadSource>("\"guess\"").is_err());
    }
}
