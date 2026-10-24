//! `[session]`: session naming and restore behaviour.

use serde::{Deserialize, Serialize};

/// Session configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    /// Session attached to when no name is given.
    pub default_name: String,
    /// Relaunch resumable agents when the server restores a session.
    pub resume_agents: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            default_name: "main".into(),
            resume_agents: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_session_is_main() {
        assert_eq!(SessionConfig::default().default_name, "main");
    }
}
