//! The attached client's main loop: apply frames, mirror modes, draw.

use std::collections::VecDeque;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use monarch_core::{Action, Key};
use monarch_proto::{ClientMsg, HostInfo, ScreenSize};
use ratatui::DefaultTerminal;

use super::{
    Applied, AttachOptions, AttachOutcome, AttachState, AttachView, ClientEvent, CopyMode,
    DragScroll, InputParams, InputThread, PROBE_TIMEOUT, Selection, ServerLink, SharedFocus,
};
use crate::{HostModeMirror, HostProbe, Picker, TuiError, WhichKeyEntry};

mod copy;
mod draw;
mod picker;
mod select;
mod sidebar;
mod which_key;

/// Minimum time between frames drawn to the host.
const FRAME: Duration = Duration::from_millis(16);
/// How long config warnings stay in the status line.
const WARNING: Duration = Duration::from_secs(8);

/// A running attach session.
pub struct AttachApp {
    ui: DefaultTerminal,
    link: ServerLink,
    events: Receiver<ClientEvent>,
    backlog: VecDeque<ClientEvent>,
    state: AttachState,
    mirror: HostModeMirror,
    host: HostInfo,
    focus: Arc<SharedFocus>,
    opts: AttachOptions,
    prefix_pending: bool,
    /// When the prefix was pressed, while it waits for a key.
    prefix_at: Option<Instant>,
    /// The key popup is up.
    popup: bool,
    which_key: Vec<WhichKeyEntry>,
    /// The open palette or go-to picker, which takes the keys.
    picker: Option<Picker>,
    /// Copy mode on the focused pane, which takes the keys.
    copy: Option<CopyMode>,
    selection: Option<Selection>,
    /// Scrolling while a selection drag is past the pane's edge.
    drag: Option<DragScroll>,
    /// The sidebar is wanted (it still hides on narrow hosts).
    sidebar: bool,
    passthrough: bool,
    notice: Option<(String, Instant)>,
    out: Vec<u8>,
    size: ScreenSize,
    dirty: bool,
    last_draw: Instant,
}

impl AttachApp {
    /// Connects, takes over the terminal and runs until detach or exit.
    ///
    /// # Errors
    /// The server is unreachable or the terminal cannot be driven.
    pub fn attach(opts: &AttachOptions) -> Result<AttachOutcome, TuiError> {
        let (link, reader) = ServerLink::connect(&opts.socket)?;
        let ui = ratatui::init();
        let result = Self::start(ui, link, reader, opts).and_then(|mut app| {
            let outcome = app.run();
            app.finish()?;
            outcome
        });
        ratatui::restore();
        result
    }

    fn start(
        mut ui: DefaultTerminal,
        link: ServerLink,
        reader: UnixStream,
        opts: &AttachOptions,
    ) -> Result<Self, TuiError> {
        let (tx, events) = mpsc::channel();
        ServerLink::spawn_reader(reader, tx.clone());
        write_host(&mut ui, HostProbe::QUERY)?;
        let focus = Arc::new(SharedFocus::default());
        spawn_input(&link, opts, Arc::clone(&focus), tx);
        let (host, backlog) = wait_for_host(&events);
        let size = introduce(&ui, &link, host, opts)?;
        let mut mirror = HostModeMirror::new(&host).with_mouse(opts.mouse);
        let mut out = Vec::new();
        mirror.enter(&mut out);
        let mut state = AttachState::new();
        state.set_left(sidebar::sidebar_cols(opts, opts.sidebar, size));
        let notice = (!opts.warnings.is_empty())
            .then(|| (opts.warnings.join("; "), Instant::now() + WARNING));
        Ok(Self {
            ui,
            link,
            events,
            backlog,
            state,
            mirror,
            host,
            focus,
            opts: opts.clone(),
            prefix_pending: false,
            prefix_at: None,
            popup: false,
            which_key: WhichKeyEntry::from_keymap(&opts.keymap),
            picker: None,
            copy: None,
            selection: None,
            drag: None,
            sidebar: opts.sidebar,
            passthrough: false,
            notice,
            out,
            size,
            dirty: true,
            last_draw: Instant::now() - FRAME,
        })
    }

    fn run(&mut self) -> Result<AttachOutcome, TuiError> {
        loop {
            let event = match self.backlog.pop_front() {
                Some(event) => Some(event),
                None => match self.events.recv_timeout(FRAME) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(AttachOutcome::ServerExited),
                },
            };
            if let Some(outcome) = event.and_then(|e| self.handle(e)) {
                return Ok(outcome);
            }
            self.check_resize()?;
            self.drag_tick();
            self.expire_notice();
            self.show_which_key();
            if self.dirty && self.last_draw.elapsed() >= FRAME {
                self.draw()?;
            }
        }
    }

    /// Applies one event; `Some` ends the session.
    fn handle(&mut self, event: ClientEvent) -> Option<AttachOutcome> {
        match event {
            ClientEvent::Server(msg) => {
                let mut applied = Applied::default();
                self.state.apply(msg, &mut applied);
                self.after_apply(applied);
            }
            ClientEvent::ServerClosed => return Some(AttachOutcome::ServerExited),
            ClientEvent::Command(key) => return self.on_command(key),
            ClientEvent::OverlayInput(bytes) => return self.on_overlay_input(&bytes),
            ClientEvent::InputClosed => return Some(AttachOutcome::Detached),
            ClientEvent::Prefix(pending) => self.on_prefix(pending),
            ClientEvent::Select(gesture) => self.on_select(gesture),
            ClientEvent::Typed => self.on_typed(),
            ClientEvent::Host(_) => {}
        }
        None
    }

    /// Runs what `key` is bound to.
    fn on_command(&mut self, key: Key) -> Option<AttachOutcome> {
        let action = self.opts.keymap.action(key)?;
        self.run_action(action)
    }

    fn after_apply(&mut self, applied: Applied) {
        for pane in applied.resync {
            let _ = self.link.send(&ClientMsg::Resync(pane));
        }
        self.out.extend(applied.host_write);
        for text in applied.clipboard {
            self.copy(text);
        }
        self.dirty |= applied.redraw;
        self.after_copy_frames(applied.found);
        if applied.redraw && self.drag.is_some() {
            self.drag_extend();
        }
        if applied.redraw {
            self.publish_panes();
        } else {
            self.focus.set(&self.state.focus_view());
        }
    }

    fn check_resize(&mut self) -> Result<(), TuiError> {
        let area = self
            .ui
            .size()
            .map_err(|e| TuiError::io("reading terminal size", e))?;
        let size = ScreenSize::new(area.width, area.height);
        if size != self.size {
            self.size = size;
            self.ui
                .autoresize()
                .map_err(|e| TuiError::io("resizing", e))?;
            self.relayout();
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), TuiError> {
        self.mirror.leave(&mut self.out);
        let out = std::mem::take(&mut self.out);
        write_host(&mut self.ui, &out)
    }
}

/// Starts the thread that reads the host terminal.
fn spawn_input(
    link: &ServerLink,
    opts: &AttachOptions,
    focus: Arc<SharedFocus>,
    events: mpsc::Sender<ClientEvent>,
) {
    InputThread::spawn(InputParams {
        link: link.clone(),
        prefix: opts.prefix,
        repeat: opts.keymap.repeatable_keys().collect(),
        repeat_window: opts.repeat,
        overlay_keys: opts
            .keymap
            .bindings()
            .filter(|(_, a)| {
                matches!(
                    a,
                    Action::CommandPalette | Action::GoToPane | Action::CopyMode
                )
            })
            .map(|(k, _)| k)
            .collect(),
        focus,
        events,
    });
}

/// Tells the server what the host can do and how big the panes are.
fn introduce(
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
fn wait_for_host(events: &Receiver<ClientEvent>) -> (HostInfo, VecDeque<ClientEvent>) {
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

fn write_host(ui: &mut DefaultTerminal, bytes: &[u8]) -> Result<(), TuiError> {
    let backend = ui.backend_mut();
    backend
        .write_all(bytes)
        .and_then(|()| backend.flush())
        .map_err(|e| TuiError::io("writing to terminal", e))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

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
