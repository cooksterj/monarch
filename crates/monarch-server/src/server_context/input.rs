//! Typing into a pane without taking the session lock, by the user or
//! through `pane.send`.

use std::time::Duration;

use monarch_core::PaneId;
use monarch_proto::{ModeFlags, PaneSend};

use super::ServerContext;
use crate::{PaneEvent, SessionState};

/// Pause between `pane.send`'s text and its Enter. Agent TUIs tell a
/// paste from typing by timing; an Enter in the same burst can be taken
/// as part of the paste (a newline) instead of a submit.
pub const ENTER_DELAY: Duration = Duration::from_millis(100);

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

    /// Types `send.text` into its pane as the user would (see
    /// [`PaneSend::typed`]), then Enter if asked, after [`ENTER_DELAY`];
    /// blocks for that long. `false` when there is no such pane.
    pub fn type_text(&self, send: &PaneSend) -> bool {
        let bracketed = {
            let state = self.lock();
            let Some(io) = state.pane(send.pane) else {
                return false;
            };
            let modes = io.shared().lock().modes();
            modes.flags.contains(ModeFlags::BRACKETED_PASTE)
        };
        let typed = send.typed(bracketed);
        if !typed.is_empty() {
            self.send_input(send.pane, typed);
        }
        if send.enter {
            std::thread::sleep(ENTER_DELAY);
            self.send_input(send.pane, PaneSend::ENTER.to_vec());
        }
        true
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

    #[test]
    fn pane_send_pastes_into_programs_that_ask_then_presses_enter() {
        let ctx = crate::test_support::context();
        let pane = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        let (tx, typed) = mpsc::channel();
        ctx.inputs.add(pane, tx);
        let send = |text: &str, enter| PaneSend {
            pane,
            text: text.into(),
            enter,
        };
        assert!(ctx.type_text(&send("a\nb", false)));
        assert_eq!(
            typed.try_recv().ok(),
            Some(b"a\rb".to_vec()),
            "no paste mode"
        );
        if let Some(io) = ctx.lock().pane(pane) {
            io.shared().lock().feed(b"\x1b[?2004h", &mut Vec::new());
        }
        assert!(ctx.type_text(&send("a\nb", true)));
        let got: Vec<Vec<u8>> = typed.try_iter().collect();
        assert_eq!(got, [b"\x1b[200~a\nb\x1b[201~".to_vec(), b"\r".to_vec()]);
        let missing = PaneSend {
            pane: PaneId(77),
            ..send("x", true)
        };
        assert!(!ctx.type_text(&missing), "no such pane");
    }
}
