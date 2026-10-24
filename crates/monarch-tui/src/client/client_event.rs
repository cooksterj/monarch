//! Everything the client's main loop reacts to, from any thread.

use monarch_proto::{HostInfo, ServerMsg};

/// One event for the main loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientEvent {
    /// A message from the server.
    Server(ServerMsg),
    /// The server connection closed.
    ServerClosed,
    /// The host probe finished.
    Host(HostInfo),
    /// The user pressed the prefix, then this key.
    Command(u8),
    /// The terminal's input closed.
    InputClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_compare_by_value() {
        assert_eq!(ClientEvent::Command(b'd'), ClientEvent::Command(b'd'));
        assert_ne!(ClientEvent::ServerClosed, ClientEvent::InputClosed);
    }
}
