//! Starting an attach session: the input thread, the host probe, the
//! first messages to the server and host-mode setup.

use std::collections::VecDeque;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use monarch_proto::{ClientMsg, HostInfo, ScreenSize};
use ratatui::DefaultTerminal;

use super::{WARNING, sidebar};
use crate::client::{
    AttachOptions, AttachState, AttachView, ClientEvent, InputKeys, InputParams, InputThread,
    PROBE_TIMEOUT, ServerLink, SharedFocus,
};
use crate::{HostModeMirror, HostProbe, TuiError};

/// What an attach session has once it is connected and introduced.
pub(super) struct Started {
    pub ui: DefaultTerminal,
    pub link: ServerLink,
    pub events: Receiver<ClientEvent>,
    pub post: mpsc::Sender<ClientEvent>,
    pub backlog: VecDeque<ClientEvent>,
    pub state: AttachState,
    pub mirror: HostModeMirror,
    pub host: HostInfo,
    pub focus: Arc<SharedFocus>,
    pub rekey: mpsc::Sender<InputKeys>,
    pub input_stop: Arc<AtomicBool>,
    pub out: Vec<u8>,
    pub size: ScreenSize,
}

impl Started {
    /// Starts the input and server threads, probes the host and
    /// introduces this client to the server.
    pub fn new(
        mut ui: DefaultTerminal,
        link: ServerLink,
        reader: UnixStream,
        opts: &AttachOptions,
    ) -> Result<Self, TuiError> {
        let (tx, events) = mpsc::channel();
        ServerLink::spawn_reader(reader, tx.clone());
        let post = tx.clone();
        write_host(&mut ui, HostProbe::QUERY)?;
        let focus = Arc::new(SharedFocus::default());
        let input_stop = Arc::new(AtomicBool::new(false));
        let rekey = spawn_input(&link, opts, Arc::clone(&focus), tx, Arc::clone(&input_stop));
        let (host, backlog) = wait_for_host(&events);
        let size = introduce(&ui, &link, host, opts)?;
        let (mirror, out) = enter_host(&host, opts);
        let mut state = AttachState::new();
        state.set_left(sidebar::sidebar_cols(opts, opts.sidebar, size));
        Ok(Self {
            ui,
            link,
            events,
            post,
            backlog,
            state,
            mirror,
            host,
            focus,
            rekey,
            input_stop,
            out,
            size,
        })
    }
}

/// Starts the thread that reads the host terminal; returns where to send
/// it new keys.
pub(super) fn spawn_input(
    link: &ServerLink,
    opts: &AttachOptions,
    focus: Arc<SharedFocus>,
    events: mpsc::Sender<ClientEvent>,
    stop: Arc<AtomicBool>,
) -> mpsc::Sender<InputKeys> {
    let (rekey_tx, rekey) = mpsc::channel();
    InputThread::spawn(InputParams {
        link: link.clone(),
        keys: InputKeys::from(opts),
        rekey,
        focus,
        events,
        stop,
    });
    rekey_tx
}

/// Config warnings as a status-line notice, if there are any.
pub(super) fn warnings_notice(opts: &AttachOptions) -> Option<(String, Instant)> {
    (!opts.warnings.is_empty()).then(|| (opts.warnings.join("; "), Instant::now() + WARNING))
}

/// The host-mode mirror for `host`, and the bytes that put the host in
/// Monarch's modes.
pub(super) fn enter_host(host: &HostInfo, opts: &AttachOptions) -> (HostModeMirror, Vec<u8>) {
    let mut mirror = HostModeMirror::new(host).with_mouse(opts.mouse);
    let mut out = Vec::new();
    mirror.enter(&mut out);
    (mirror, out)
}

/// Tells the server what the host can do and how big the panes are.
pub(super) fn introduce(
    ui: &DefaultTerminal,
    link: &ServerLink,
    host: HostInfo,
    opts: &AttachOptions,
) -> Result<ScreenSize, TuiError> {
    let area = ui
        .size()
        .map_err(|e| TuiError::io("reading terminal size", e))?;
    let size = ScreenSize::new(area.width, area.height);
    link.send(&ClientMsg::HostInfo(host))?;
    let left = sidebar::sidebar_cols(opts, opts.sidebar, size);
    link.send(&ClientMsg::Resize(AttachView::pane_area(size, left)))?;
    Ok(size)
}

/// Waits for the host probe, keeping every other event for later.
pub(super) fn wait_for_host(events: &Receiver<ClientEvent>) -> (HostInfo, VecDeque<ClientEvent>) {
    let mut backlog = VecDeque::new();
    let deadline = Instant::now() + PROBE_TIMEOUT;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match events.recv_timeout(left) {
            Ok(ClientEvent::Host(info)) => return (info, backlog),
            Ok(other) => backlog.push_back(other),
            Err(_) => break,
        }
    }
    (HostInfo::default(), backlog)
}

pub(super) fn write_host(ui: &mut DefaultTerminal, bytes: &[u8]) -> Result<(), TuiError> {
    let backend = ui.backend_mut();
    backend
        .write_all(bytes)
        .and_then(|()| backend.flush())
        .map_err(|e| TuiError::io("writing to terminal", e))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use monarch_core::Key;
    use monarch_proto::ServerMsg;

    use super::*;

    #[test]
    fn events_before_the_probe_answer_are_kept_in_order() {
        let (tx, rx) = mpsc::channel();
        tx.send(ClientEvent::Server(ServerMsg::Pong(1)))
            .expect("send");
        tx.send(ClientEvent::Command(Key::Char('x'))).expect("send");
        tx.send(ClientEvent::Host(HostInfo::default()))
            .expect("send");
        let (_, backlog) = wait_for_host(&rx);
        assert_eq!(
            Vec::from(backlog),
            [
                ClientEvent::Server(ServerMsg::Pong(1)),
                ClientEvent::Command(Key::Char('x'))
            ]
        );
    }

    #[test]
    fn a_silent_host_gets_defaults() {
        let (_tx, rx) = mpsc::channel();
        let (host, backlog) = wait_for_host(&rx);
        assert_eq!(host, HostInfo::default());
        assert!(backlog.is_empty());
    }
}
