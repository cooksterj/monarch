//! The clients attached to the session: joining, leaving and what each is
//! sent.

use monarch_core::ClientId;
use monarch_proto::ServerMsg;
use tokio::sync::mpsc::Sender;

use super::SessionState;
use crate::ClientHandle;

impl SessionState {
    /// Registers a client; it gets the layout and snapshots on the next frame.
    pub fn add_client(&mut self, lane: Sender<Vec<ServerMsg>>) -> ClientId {
        let id = self.next_client;
        self.next_client = id.next();
        let mut client = ClientHandle::new(id, lane);
        client.resync(self.visible_panes());
        self.clients.insert(id, client);
        id
    }

    /// Forgets a client; the window takes the size of the client that
    /// reported one most recently, so a smaller terminal left attached is
    /// not drawn clipped.
    pub fn remove_client(&mut self, id: ClientId) {
        self.clients.remove(&id);
        if let Some(size) = self.latest_window() {
            self.resize(size);
        }
    }

    /// A client, if still attached.
    pub fn client_mut(&mut self, id: ClientId) -> Option<&mut ClientHandle> {
        self.clients.get_mut(&id)
    }

    /// Number of attached clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Sends one message to every client.
    pub fn broadcast(&mut self, msg: &ServerMsg) {
        self.clients
            .values_mut()
            .for_each(|c| c.push(vec![msg.clone()]));
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::test_support::FakePane;

    #[test]
    fn clients_are_counted_and_broadcast_to_until_they_leave() {
        let mut state = SessionState::default();
        state.insert_pane(monarch_core::PaneId(1), FakePane::shell());
        let (lane, mut rx) = mpsc::channel(64);
        let id = state.add_client(lane);
        assert_eq!(state.client_count(), 1);
        while rx.try_recv().is_ok() {}
        state.broadcast(&ServerMsg::Title {
            pane: monarch_core::PaneId(1),
            title: Some("t".into()),
        });
        assert!(rx.try_recv().is_ok(), "broadcast reaches the client");
        state.remove_client(id);
        assert_eq!(state.client_count(), 0);
        assert!(state.client_mut(id).is_none(), "gone");
    }
}
