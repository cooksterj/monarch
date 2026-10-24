//! Messages from an attached client to the server.

use monarch_core::{Action, PaneId};
use serde::{Deserialize, Serialize};

use super::HostInfo;
use crate::ScreenSize;

/// One client → server attach-stream message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ClientMsg {
    /// Bytes for a pane's program, already filtered for that pane.
    Input {
        /// Target pane.
        pane: PaneId,
        /// Exact bytes to write to its PTY.
        bytes: Vec<u8>,
    },
    /// The client's window size changed.
    Resize(ScreenSize),
    /// What the client learned by probing its host terminal.
    HostInfo(HostInfo),
    /// Move input focus to a pane.
    Focus(PaneId),
    /// Scroll a pane's view: positive into history, negative toward live.
    Scroll {
        /// Target pane.
        pane: PaneId,
        /// Lines to scroll.
        lines: i32,
    },
    /// The client lost track of a pane (sequence gap); send a snapshot.
    Resync(PaneId),
    /// Round-trip probe; the server answers with `Pong` and the same token.
    Ping(u64),
    /// The client is leaving; the session keeps running.
    Detach,
    /// Run a session command (split, focus, tabs…).
    Action(Action),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_keeps_bytes_verbatim() {
        let msg = ClientMsg::Input {
            pane: PaneId(1),
            bytes: b"\x1b[A".to_vec(),
        };
        assert!(matches!(msg, ClientMsg::Input { ref bytes, .. } if bytes == b"\x1b[A"));
    }
}
