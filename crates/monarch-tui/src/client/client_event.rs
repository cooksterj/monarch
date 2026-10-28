//! Everything the client's main loop reacts to, from any thread.

use monarch_core::Key;
use monarch_proto::{HostInfo, ServerMsg};

use super::{SelectEvent, SessionEntry};

/// One event for the main loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientEvent {
    /// A message from the server.
    Server(ServerMsg),
    /// The server connection closed.
    ServerClosed,
    /// The host probe finished.
    Host(HostInfo),
    /// The prefix was pressed (`true`) or its command key arrived (`false`).
    Prefix(bool),
    /// The user pressed the prefix, then this key.
    Command(Key),
    /// Host input while a pop-up is open, for it to decode.
    OverlayInput(Vec<u8>),
    /// A mouse selection gesture.
    Select(SelectEvent),
    /// A right-click asked for the context menu at this host cell.
    Menu {
        /// Host column.
        col: u16,
        /// Host row.
        row: u16,
    },
    /// The wheel turned over the sidebar: scroll it by this many lines
    /// (negative is up).
    SidebarScroll(i32),
    /// Keys were sent to the focused pane (a selection ends).
    Typed,
    /// The running sessions, listed for the session picker.
    Sessions(Vec<SessionEntry>),
    /// The terminal's input closed.
    InputClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_compare_by_value() {
        let d = ClientEvent::Command(Key::Char('d'));
        assert_eq!(d.clone(), d);
        assert_ne!(ClientEvent::ServerClosed, ClientEvent::InputClosed);
    }
}
