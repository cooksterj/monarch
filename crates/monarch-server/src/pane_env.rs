//! Session facts placed in every pane's environment.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// Session facts every pane's environment carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneEnv {
    /// Session name.
    pub session: String,
    /// The server's socket.
    pub socket: PathBuf,
    /// The user's `[terminal].env`, set before Monarch's own variables.
    pub extra: BTreeMap<String, String>,
}

impl PaneEnv {
    /// The environment of session `session` served at `socket`, with no
    /// extra variables.
    #[must_use]
    pub fn new(session: impl Into<String>, socket: impl Into<PathBuf>) -> Self {
        Self {
            session: session.into(),
            socket: socket.into(),
            extra: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_session_and_socket() {
        let env = PaneEnv::new("main", "/tmp/x.sock");
        assert_eq!(env.session, "main");
        assert!(env.socket.ends_with("x.sock"));
        assert!(env.extra.is_empty());
    }
}
