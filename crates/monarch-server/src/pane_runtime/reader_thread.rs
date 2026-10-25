//! Reads PTY output into the emulator; one std thread per pane.

use std::io::{ErrorKind, Read};
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use monarch_core::PaneId;
use monarch_detect::OscSignal;
use monarch_term::EngineEvent;
use portable_pty::Child;
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedSender;

use crate::{PaneEvent, PaneShared};

/// Largest single read; bounds lock hold time per feed.
const READ_CHUNK: usize = 64 * 1024;

/// What the reader thread owns.
pub(super) struct ReaderParams {
    pub pane: PaneId,
    pub reader: Box<dyn Read + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub shared: Arc<PaneShared>,
    /// The pane's input channel: query replies go here so they stay in
    /// order with user input.
    pub replies: Sender<Vec<u8>>,
    pub events: UnboundedSender<PaneEvent>,
    pub wake: Arc<Notify>,
}

/// `read` → feed emulator → route events → mark dirty, until EOF; then
/// reaps the child and reports its exit.
pub(super) struct ReaderThread;

impl ReaderThread {
    pub(super) fn spawn(p: ReaderParams) -> std::io::Result<JoinHandle<()>> {
        thread::Builder::new()
            .name(format!("monarch-read-{}", p.pane))
            .spawn(move || run(p))
    }
}

fn run(mut p: ReaderParams) {
    let mut buf = vec![0u8; READ_CHUNK];
    let mut events = Vec::new();
    let mut reported_sync: Option<Instant> = None;
    loop {
        let n = match p.reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let deadline = {
            let mut term = p.shared.lock();
            term.feed(&buf[..n], &mut events);
            term.sync_deadline()
        };
        events.drain(..).for_each(|e| route(&p, e));
        if let Some(deadline) = deadline.filter(|d| Some(*d) != reported_sync) {
            let pane = p.pane;
            let _ = p.events.send(PaneEvent::SyncPending { pane, deadline });
        }
        reported_sync = deadline;
        if p.shared.mark_dirty() {
            p.wake.notify_one();
        }
    }
    let code = p.child.wait().ok().map(|status| status.exit_code());
    let _ = p.events.send(PaneEvent::Exited { pane: p.pane, code });
}

fn route(p: &ReaderParams, event: EngineEvent) {
    let pane = p.pane;
    let event = match event {
        EngineEvent::Reply(bytes) => {
            let _ = p.replies.send(bytes);
            return;
        }
        EngineEvent::Title(title) => PaneEvent::Title { pane, title },
        EngineEvent::Bell => PaneEvent::Bell(pane),
        EngineEvent::ClipboardStore(text) => PaneEvent::Clipboard { pane, text },
        EngineEvent::Osc(osc) => match OscSignal::parse(osc.number, &osc.payload_str()) {
            Some(signal) => PaneEvent::Signal { pane, signal },
            // Cwd (7) and prompt marks (133) aren't used yet.
            None => return,
        },
        _ => return,
    };
    let _ = p.events.send(event);
}
