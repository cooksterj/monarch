//! What `monarch attach` needs to know.

use std::path::PathBuf;

/// Where the session is and which key is the prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachOptions {
    /// The session server's socket.
    pub socket: PathBuf,
    /// The prefix control byte (`0x02` for Ctrl-b).
    pub prefix: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_socket_and_prefix() {
        let opts = AttachOptions {
            socket: "/tmp/m.sock".into(),
            prefix: 0x02,
        };
        assert_eq!(opts.prefix, 0x02);
        assert!(opts.socket.ends_with("m.sock"));
    }
}
