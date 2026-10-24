//! Session facts placed in every pane's environment.

use std::path::PathBuf;

/// Session facts every pane's environment carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneEnv {
    /// Session name.
    pub session: String,
    /// The server's socket.
    pub socket: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_session_and_socket() {
        let env = PaneEnv {
            session: "main".into(),
            socket: "/tmp/x.sock".into(),
        };
        assert_eq!(env.session, "main");
        assert!(env.socket.ends_with("x.sock"));
    }
}
