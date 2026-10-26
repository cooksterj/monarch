//! Typing into a pane without taking the session lock.

use monarch_core::PaneId;

use super::ServerContext;
use crate::{PaneEvent, SessionState};

impl ServerContext {
    /// Sends what the user typed to `pane`'s program straight away, then
    /// lets the event loop note whether it answered the pane's agent.
    /// Panes without an input channel (test fakes) take the slow path.
    pub fn send_input(&self, pane: PaneId, bytes: Vec<u8>) {
        let reply = SessionState::is_reply(&bytes);
        if let Err(bytes) = self.inputs.send(pane, bytes)
            && let Some(io) = self.lock().pane(pane)
        {
            io.send(bytes);
        }
        if reply {
            let _ = self.events.send(PaneEvent::Replied(pane));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::{PaneEnv, PaneSpec, test_support::FakePane};

    #[test]
    fn typing_reaches_the_pane_and_only_replies_reach_the_event_loop() {
        let (events, mut rx) = unbounded_channel();
        let env = PaneEnv {
            session: "t".into(),
            socket: "/tmp/t.sock".into(),
        };
        let ctx = ServerContext::new(env, events, 100);
        let fake = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        ctx.send_input(fake, b"\x1b[A".to_vec());
        assert!(rx.try_recv().is_err(), "an arrow is not a reply");
        ctx.send_input(fake, b"y".to_vec());
        assert_eq!(rx.try_recv().ok(), Some(PaneEvent::Replied(fake)));

        let (tx, typed) = mpsc::channel();
        ctx.inputs.add(PaneId(99), tx);
        ctx.send_input(PaneId(99), b"ls\r".to_vec());
        assert_eq!(typed.try_recv().ok(), Some(b"ls\r".to_vec()), "routed");
        ctx.forget_pane(PaneId(99));
        ctx.send_input(PaneId(99), b"gone".to_vec());
        assert!(typed.try_recv().is_err(), "forgotten with its pane");
    }
}
