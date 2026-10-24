//! How to start a session server.

use std::path::PathBuf;

use crate::{PaneSpec, StateFile};

/// Session name, socket, initial panes and where state is kept.
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
    /// Where the session is saved for restore; `None` keeps no state.
    pub state: Option<StateFile>,
    /// Start from the saved session, if there is one, instead of `panes`.
    pub restore: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            session: "main".into(),
            socket: None,
            panes: vec![PaneSpec::default()],
            scrollback: 10_000,
            state: None,
            restore: false,
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
        assert!(
            config.state.is_none() && !config.restore,
            "no state unless asked"
        );
    }
}
