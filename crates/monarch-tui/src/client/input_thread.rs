//! Raw terminal input → server, on its own std thread (G1).
//!
//! Bytes go host probe → prefix parser → per-pane filter → socket, never
//! through the render loop, so typing latency does not depend on drawing.

use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use monarch_proto::ClientMsg;

use super::{ClientEvent, ServerLink, SharedFocus};
use crate::{HostInputFilter, HostProbe, InputEvent, PrefixParser};

/// How long to wait for the host to answer the startup probe.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
/// How long keys typed before the first layout wait for a focused pane.
const FOCUS_TIMEOUT: Duration = Duration::from_secs(2);

/// What the input thread owns.
pub struct InputParams {
    /// Where pane input goes.
    pub link: ServerLink,
    /// The prefix control byte.
    pub prefix: u8,
    /// Which pane has focus and its modes.
    pub focus: Arc<SharedFocus>,
    /// Commands, probe results and end of input go to the main loop.
    pub events: Sender<ClientEvent>,
}

/// Spawns the input thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct InputThread;

impl InputThread {
    /// Starts reading the terminal.
    pub fn spawn(params: InputParams) {
        let _ = thread::Builder::new()
            .name("monarch-input".into())
            .spawn(move || run(&params, std::io::stdin().lock()));
    }
}

fn run(p: &InputParams, mut stdin: impl Read) {
    let mut buf = [0u8; 4096];
    let mut probe = HostProbe::new();
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let mut parser = PrefixParser::new(p.prefix);
    let mut filter = HostInputFilter::new();
    let (mut input, mut events) = (Vec::new(), Vec::new());
    let mut pending = false;
    while let Ok(n @ 1..) = stdin.read(&mut buf) {
        input.clear();
        let probing = !probe.is_done();
        if probe.feed(&buf[..n], &mut input) && probing {
            let _ = p.events.send(ClientEvent::Host(probe.info()));
        } else if !probe.is_done() && Instant::now() > deadline {
            probe.abandon(&mut input);
        }
        parser.feed(&input, &mut events);
        for event in events.drain(..) {
            dispatch(p, &mut filter, event);
        }
        if parser.awaiting_command() != pending {
            pending = parser.awaiting_command();
            let _ = p.events.send(ClientEvent::Prefix(pending));
        }
    }
    let _ = p.events.send(ClientEvent::InputClosed);
}

fn dispatch(p: &InputParams, filter: &mut HostInputFilter, event: InputEvent) {
    match event {
        InputEvent::Forward(raw) => {
            let Some(pane) = p.focus.wait_pane(FOCUS_TIMEOUT) else {
                return;
            };
            let mut bytes = Vec::with_capacity(raw.len());
            filter.filter(&raw, p.focus.modes(), &mut bytes);
            if !bytes.is_empty() {
                let _ = p.link.send(&ClientMsg::Input { pane, bytes });
            }
        }
        InputEvent::Command(key) => {
            let _ = p.events.send(ClientEvent::Command(key));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    use bytes::BytesMut;
    use monarch_core::PaneId;
    use monarch_proto::{FrameCodec, ModeFlags, ServerMsg};

    use super::*;

    #[test]
    fn probe_reply_keys_and_commands_are_routed() {
        let (ours, mut theirs) = UnixStream::pair().expect("pair");
        let (events, rx) = mpsc::channel();
        let focus = Arc::new(SharedFocus::default());
        focus.set(Some(PaneId(1)), ModeFlags::empty());
        let params = InputParams {
            link: ServerLink::from_stream(ours),
            prefix: 0x02,
            focus,
            events,
        };
        run(&params, &b"\x1b[?62;22chi\x02d"[..]);
        drop(params);

        assert!(matches!(rx.recv(), Ok(ClientEvent::Host(_))));
        assert_eq!(rx.recv().ok(), Some(ClientEvent::Command(b'd')));
        assert_eq!(rx.recv().ok(), Some(ClientEvent::InputClosed));
        let mut wire = Vec::new();
        theirs.read_to_end(&mut wire).expect("read");
        let codec = FrameCodec::<ClientMsg, ServerMsg>::default();
        let msg = codec
            .decode_from(&mut BytesMut::from(&wire[..]))
            .expect("frame");
        assert_eq!(
            msg,
            Some(ClientMsg::Input {
                pane: PaneId(1),
                bytes: b"hi".to_vec()
            })
        );
    }

    #[test]
    fn input_without_a_focused_pane_is_dropped() {
        let (ours, mut theirs) = UnixStream::pair().expect("pair");
        let (events, rx) = mpsc::channel();
        let focus = Arc::new(SharedFocus::default());
        let params = InputParams {
            link: ServerLink::from_stream(ours),
            prefix: 0x02,
            focus,
            events,
        };
        run(&params, &b"\x1b[?62;22cx"[..]);
        drop(params);
        assert!(matches!(rx.recv(), Ok(ClientEvent::Host(_))));
        assert_eq!(rx.recv().ok(), Some(ClientEvent::InputClosed));
        let mut wire = Vec::new();
        theirs.read_to_end(&mut wire).expect("read");
        assert!(wire.is_empty());
    }
}
