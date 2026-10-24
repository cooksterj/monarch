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
use crate::{
    HostInput, HostInputFilter, HostProbe, InputEvent, MouseEvent, MouseExtractor, MouseKind,
    PrefixParser,
};

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
    /// A click focused another pane; its drags and release go nowhere.
    focusing: bool,
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
                    HostInput::Mouse(event) => route_mouse(p, pipe, &event),
                }
            }
        }
        InputEvent::Command(key) => {
            let _ = p.events.send(ClientEvent::Command(key));
        }
    }
}

/// A press on another pane focuses it (and nothing else); everything
/// else goes through the router for the focused pane.
fn route_mouse(p: &InputParams, pipe: &mut Pipeline, event: &MouseEvent) {
    if pipe.focusing {
        pipe.focusing = !matches!(event.kind, MouseKind::Release(_));
        return;
    }
    let view = p.focus.view();
    if matches!(event.kind, MouseKind::Press(_))
        && view.local(event.col, event.row).is_none()
        && let Some(other) = p.focus.pane_at(event.col, event.row)
    {
        pipe.focusing = true;
        let _ = p.link.send(&ClientMsg::Focus(other));
        return;
    }
    let action = pipe.router.route(event, &view, Instant::now());
    send_mouse(p, action);
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
    use monarch_core::{PaneId, Rect};
    use monarch_proto::{FrameCodec, ModeFlags, ServerMsg};

    use super::*;
    use crate::client::FocusView;

    /// Runs the thread over `input` and returns everything it sent.
    fn sent(focus: FocusView, input: &[u8]) -> Vec<ClientMsg> {
        sent_with_panes(focus, &[], input)
    }

    /// Like [`sent`], with other panes laid out beside the focused one.
    fn sent_with_panes(focus: FocusView, panes: &[(PaneId, Rect)], input: &[u8]) -> Vec<ClientMsg> {
        let (ours, mut theirs) = UnixStream::pair().expect("pair");
        let (events, _rx) = mpsc::channel();
        let shared = Arc::new(SharedFocus::default());
        shared.set(&focus);
        shared.set_panes(panes.to_vec());
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
    fn a_click_on_another_pane_focuses_it_and_nothing_else() {
        let focus = FocusView {
            pane: Some(PaneId(1)),
            cols: 10,
            rows: 5,
            modes: ModeFlags::MOUSE_CLICK | ModeFlags::SGR_MOUSE,
            ..FocusView::default()
        };
        let panes = [
            (PaneId(1), Rect::new(0, 0, 10, 5)),
            (PaneId(2), Rect::new(11, 0, 10, 5)),
        ];
        let click = b"\x1b[?62;22c\x1b[<0;13;2M\x1b[<32;3;2M\x1b[<0;3;2m\x1b[<0;2;2M";
        let msgs = sent_with_panes(focus, &panes, click);
        assert_eq!(msgs[0], ClientMsg::Focus(PaneId(2)));
        assert_eq!(
            msgs.len(),
            2,
            "the drag and release are swallowed: {msgs:?}"
        );
        assert!(
            matches!(
                &msgs[1],
                ClientMsg::Input {
                    pane: PaneId(1),
                    ..
                }
            ),
            "the next click goes to the focused pane as usual"
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
        assert_eq!(
            rx.recv().ok(),
            Some(ClientEvent::Command(monarch_core::Key::Char('d')))
        );
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
