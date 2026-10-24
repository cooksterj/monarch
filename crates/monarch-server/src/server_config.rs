//! How to start a session server.

use std::path::PathBuf;

use crate::PaneSpec;

/// Session name, socket and initial panes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Session name (also the socket file name).
    pub session: String,
    /// Socket path; derived from the session when `None`.
    pub socket: Option<PathBuf>,
    /// Panes to start with (at least one is needed to stay alive).
    pub panes: Vec<PaneSpec>,
    /// Scrollback lines per pane.
    pub scrollback: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            session: "main".into(),
            socket: None,
            panes: vec![PaneSpec::default()],
            scrollback: 10_000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_one_shell_in_main() {
        let config = ServerConfig::default();
        assert_eq!(config.session, "main");
        assert_eq!(config.panes, [PaneSpec::default()]);
    }
}
