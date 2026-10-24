//! Messages from the server to an attached client.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

use super::{Delta, LayoutMsg, Snapshot, StyleDef};

/// One server → client attach-stream message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ServerMsg {
    /// The pane arrangement changed.
    Layout(LayoutMsg),
    /// A pane's full screen.
    Snapshot(Snapshot),
    /// A pane's changed rows.
    Delta(Delta),
    /// A new style id, valid from here on.
    DefineStyle(StyleDef),
    /// Forget every style id (the server's table started over).
    ResetStyles,
    /// A pane's program set or cleared its title.
    Title {
        /// The pane.
        pane: PaneId,
        /// The new title.
        title: Option<String>,
    },
    /// Bytes for the host terminal itself (bell).
    HostWrite(Vec<u8>),
    /// A visible pane's program copied text (OSC 52); the client puts it
    /// on the user's clipboard. Reading the clipboard is never offered.
    Clipboard(String),
    /// A pane's program exited.
    PaneExited {
        /// The pane.
        pane: PaneId,
    },
    /// Answer to [`super::ClientMsg::Ping`], echoing its token.
    Pong(u64),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_compare_by_content() {
        assert_eq!(ServerMsg::Pong(3), ServerMsg::Pong(3));
        assert_ne!(ServerMsg::ResetStyles, ServerMsg::HostWrite(vec![7]));
        assert_ne!(
            ServerMsg::Clipboard("a".into()),
            ServerMsg::Clipboard("b".into())
        );
    }
}
