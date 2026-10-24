//! Raw terminal input → server, on its own std thread (G1).
//!
//! Bytes go host probe → prefix parser → mouse extractor → per-pane filter
//! (keys) or mouse router (reports) → socket, never through the render
//! loop, so typing latency does not depend on drawing.

use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

use monarch_proto::ClientMsg;

use super::{ClientEvent, MouseAction, MouseRouter, ServerLink, SharedFocus};
use crate::{HostInput, HostInputFilter, HostProbe, InputEvent, MouseExtractor, PrefixParser};

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

/// Scrolls to live output: more than any scrollback holds.
const TO_BOTTOM: i32 = -i32::MAX;

/// Per-stream state between the prefix parser and the socket.
#[derive(Default)]
struct Pipeline {
    mice: MouseExtractor,
    filter: HostInputFilter,
    router: MouseRouter,
    pieces: Vec<HostInput>,
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
    let mut pipeline = Pipeline::default();
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
            dispatch(p, &mut pipeline, event);
        }
        if parser.awaiting_command() != pending {
            pending = parser.awaiting_command();
            let _ = p.events.send(ClientEvent::Prefix(pending));
        }
    }
    let _ = p.events.send(ClientEvent::InputClosed);
}

fn dispatch(p: &InputParams, pipe: &mut Pipeline, event: InputEvent) {
    match event {
        InputEvent::Forward(raw) => {
            pipe.mice.split(&raw, &mut pipe.pieces);
            for piece in std::mem::take(&mut pipe.pieces) {
                match piece {
                    HostInput::Keys(keys) => send_keys(p, &mut pipe.filter, &keys),
                    HostInput::Mouse(event) => {
                        let action = pipe.router.route(&event, &p.focus.view(), Instant::now());
                        send_mouse(p, action);
                    }
                }
            }
        }
        InputEvent::Command(key) => {
            let _ = p.events.send(ClientEvent::Command(key));
        }
    }
}

/// Keys go to the focused pane; typing while scrolled back returns to live
/// output first, as in a bare terminal.
fn send_keys(p: &InputParams, filter: &mut HostInputFilter, raw: &[u8]) {
    let Some(pane) = p.focus.wait_pane(FOCUS_TIMEOUT) else {
        return;
    };
    let mut bytes = Vec::with_capacity(raw.len());
    filter.filter(raw, p.focus.modes(), &mut bytes);
    if bytes.is_empty() {
        return;
    }
    if p.focus.scrolled() {
        p.focus.clear_scrolled();
        let _ = p.link.send(&ClientMsg::Scroll {
            pane,
            lines: TO_BOTTOM,
        });
    }
    let _ = p.link.send(&ClientMsg::Input { pane, bytes });
    let _ = p.events.send(ClientEvent::Typed);
}

fn send_mouse(p: &InputParams, action: MouseAction) {
    let Some(pane) = p.focus.pane() else {
        return;
    };
    let msg = match action {
        MouseAction::Forward(bytes) => ClientMsg::Input { pane, bytes },
        MouseAction::Scroll(lines) => ClientMsg::Scroll { pane, lines },
        MouseAction::Select(gesture) => {
            let _ = p.events.send(ClientEvent::Select(gesture));
            return;
        }
        MouseAction::Ignore => return,
    };
    let _ = p.link.send(&msg);
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixStream;
    use std::sync::mpsc;

    use bytes::BytesMut;
    use monarch_core::PaneId;
    use monarch_proto::{FrameCodec, ServerMsg};

    use super::*;
    use crate::client::FocusView;

    /// Runs the thread over `input` and returns everything it sent.
    fn sent(focus: FocusView, input: &[u8]) -> Vec<ClientMsg> {
        let (ours, mut theirs) = UnixStream::pair().expect("pair");
        let (events, _rx) = mpsc::channel();
        let shared = Arc::new(SharedFocus::default());
        shared.set(&focus);
        let params = InputParams {
            link: ServerLink::from_stream(ours),
            prefix: 0x02,
            focus: shared,
            events,
        };
        run(&params, input);
        drop(params);
        let mut wire = Vec::new();
        theirs.read_to_end(&mut wire).expect("read");
        let codec = FrameCodec::<ClientMsg, ServerMsg>::default();
        let mut buf = BytesMut::from(&wire[..]);
        std::iter::from_fn(|| codec.decode_from(&mut buf).expect("frame")).collect()
    }

    #[test]
    fn wheel_scrolls_and_typing_returns_to_live_output() {
        let pane = PaneId(1);
        let focus = FocusView {
            pane: Some(pane),
            cols: 80,
            rows: 23,
            scrolled: true,
            ..FocusView::default()
        };
        let msgs = sent(focus, b"\x1b[?62;22c\x1b[<64;5;5Mx");
        assert_eq!(
            msgs,
            [
                ClientMsg::Scroll { pane, lines: 3 },
                ClientMsg::Scroll {
                    pane,
                    lines: TO_BOTTOM
                },
                ClientMsg::Input {
                    pane,
                    bytes: b"x".to_vec()
                },
            ]
        );
    }

    #[test]
    fn mouse_reports_never_reach_a_pane_that_did_not_ask() {
        let focus = FocusView {
            pane: Some(PaneId(1)),
            cols: 80,
            rows: 23,
            ..FocusView::default()
        };
        let msgs = sent(focus, b"\x1b[?62;22ca\x1b[<0;5;5M\x1b[<0;5;5mb");
        let typed: Vec<u8> = msgs
            .iter()
            .flat_map(|m| match m {
                ClientMsg::Input { bytes, .. } => bytes.clone(),
                _ => Vec::new(),
            })
            .collect();
        assert_eq!(typed, b"ab");
    }

    #[test]
    fn probe_reply_keys_and_commands_are_routed() {
        let (ours, mut theirs) = UnixStream::pair().expect("pair");
        let (events, rx) = mpsc::channel();
        let focus = Arc::new(SharedFocus::default());
        focus.set(&FocusView {
            pane: Some(PaneId(1)),
            ..FocusView::default()
        });
        let params = InputParams {
            link: ServerLink::from_stream(ours),
            prefix: 0x02,
            focus,
            events,
        };
        run(&params, &b"\x1b[?62;22chi\x02d"[..]);
        drop(params);

        assert!(matches!(rx.recv(), Ok(ClientEvent::Host(_))));
        assert_eq!(rx.recv().ok(), Some(ClientEvent::Typed), "after `hi`");
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
