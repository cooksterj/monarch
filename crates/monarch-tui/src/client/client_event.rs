//! Everything the client's main loop reacts to, from any thread.

use monarch_core::Key;
use monarch_proto::{HostInfo, ServerMsg};

use super::SelectEvent;

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
    /// Keys were sent to the focused pane (a selection ends).
    Typed,
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
