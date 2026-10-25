//! Where a pane's agent status came from.

use serde::{Deserialize, Serialize};

/// The signal behind a pane's current status, strongest first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StatusSource {
    /// The agent's hooks report it (authoritative).
    Hooks,
    /// Its screen, read with the agent's detection manifest.
    Screen,
    /// An OSC notification or progress report.
    Notification,
    /// Monarch launched it and nothing has reported since.
    Launch,
    /// Nothing: a plain shell or program.
    #[default]
    None,
}

impl StatusSource {
    /// How `monarch agent explain` says it.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Hooks => "its hooks",
            Self::Screen => "its screen",
            Self::Notification => "a terminal notification",
            Self::Launch => "Monarch launching it",
            Self::None => "nothing (not an agent)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_snake_case_and_describes_itself() {
        let json = serde_json::to_string(&StatusSource::Notification).expect("json");
        assert_eq!(json, "\"notification\"");
        assert_eq!(StatusSource::default().describe(), "nothing (not an agent)");
        assert!(serde_json::from_str::<StatusSource>("\"guess\"").is_err());
    }
}
