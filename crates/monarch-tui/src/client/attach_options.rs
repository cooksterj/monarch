//! What `monarch attach` needs to know.

use std::path::PathBuf;
use std::time::Duration;

use monarch_core::{Keymap, MouseMode};

/// Where the session is, which key is the prefix and how to title the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachOptions {
    /// The session server's socket.
    pub socket: PathBuf,
    /// Session name, shown in the status line and window title.
    pub session: String,
    /// The prefix control byte (`0x02` for Ctrl-b).
    pub prefix: u8,
    /// The prefix as the user wrote it, for hints (`ctrl+b`).
    pub prefix_label: String,
    /// `[ui].window_title` template.
    pub window_title: String,
    /// `[ui].mouse` capture policy.
    pub mouse: MouseMode,
    /// What each key after the prefix does.
    pub keymap: Keymap,
    /// How long the prefix waits before the key popup appears.
    pub which_key_delay: Duration,
    /// How long a repeatable key keeps working without the prefix.
    pub repeat: Duration,
    /// Problems with the config, shown once the client is up.
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_socket_and_prefix() {
        let opts = AttachOptions {
            socket: "/tmp/m.sock".into(),
            session: "main".into(),
            prefix: 0x02,
            prefix_label: "ctrl+b".into(),
            window_title: "{title}".into(),
            mouse: MouseMode::Auto,
            keymap: Keymap::default(),
            which_key_delay: Duration::from_millis(300),
            repeat: Duration::from_millis(500),
            warnings: Vec::new(),
        };
        assert_eq!(opts.prefix, 0x02);
        assert!(opts.socket.ends_with("m.sock"));
    }
}
