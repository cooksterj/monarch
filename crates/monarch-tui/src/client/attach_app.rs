//! The attached client's main loop: apply frames, mirror modes, draw.

use std::collections::VecDeque;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use monarch_proto::{ClientMsg, HostInfo, ScreenSize};
use ratatui::DefaultTerminal;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};

use super::{
    Applied, AttachOptions, AttachOutcome, AttachState, AttachView, ClientEvent, InputParams,
    InputThread, PROBE_TIMEOUT, Selection, ServerLink, SharedFocus, WindowTitle,
};
use crate::{HostModeMirror, HostProbe, StatusLine, TuiError};

mod select;

/// Minimum time between frames drawn to the host.
const FRAME: Duration = Duration::from_millis(16);
/// Detach key pressed after the prefix.
const DETACH: u8 = b'd';
/// Mouse passthrough toggle pressed after the prefix.
const MOUSE_TOGGLE: u8 = b'm';

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
    selection: Option<Selection>,
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
        InputThread::spawn(InputParams {
            link: link.clone(),
            prefix: opts.prefix,
            focus: Arc::clone(&focus),
            events: tx,
        });
        let (host, backlog) = wait_for_host(&events);
        let size = introduce(&ui, &link, host)?;
        let mut mirror = HostModeMirror::new(&host).with_mouse(opts.mouse);
        let mut out = Vec::new();
        mirror.enter(&mut out);
        let state = AttachState::new();
        let last_draw = Instant::now() - FRAME;
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
            selection: None,
            passthrough: false,
            notice: None,
            out,
            size,
            dirty: true,
            last_draw,
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
            self.expire_notice();
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
            ClientEvent::Command(DETACH) => {
                let _ = self.link.send(&ClientMsg::Detach);
                return Some(AttachOutcome::Detached);
            }
            ClientEvent::InputClosed => return Some(AttachOutcome::Detached),
            ClientEvent::Prefix(pending) => {
                self.prefix_pending = pending;
                self.dirty = true;
            }
            ClientEvent::Command(MOUSE_TOGGLE) => self.toggle_mouse(),
            ClientEvent::Select(gesture) => self.on_select(gesture),
            ClientEvent::Typed => self.on_typed(),
            ClientEvent::Command(_) | ClientEvent::Host(_) => {}
        }
        None
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
        self.focus.set(&self.state.focus_view());
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
            self.link
                .send(&ClientMsg::Resize(AttachView::pane_area(size)))?;
            self.dirty = true;
        }
        Ok(())
    }

    /// Mirrors the focused pane onto the host, then draws every visible
    /// pane, inside one synchronized update when the host supports it.
    fn draw(&mut self) -> Result<(), TuiError> {
        let focus = self.state.focus();
        if let Some(screen) = focus.and_then(|p| self.state.screen(p)) {
            self.mirror
                .sync(&screen.modes, &screen.cursor, &mut self.out);
        }
        let title = WindowTitle::format(
            &self.opts.window_title,
            &self.opts.session,
            focus,
            self.state.focused_title(),
        );
        self.mirror.sync_title(Some(&title), &mut self.out);
        let io = |e| TuiError::io("drawing", e);
        if self.host.sync_output {
            queue!(self.ui.backend_mut(), BeginSynchronizedUpdate).map_err(io)?;
        }
        self.ui.backend_mut().write_all(&self.out).map_err(io)?;
        self.out.clear();
        let view = AttachView {
            state: &self.state,
            status: StatusLine {
                session: &self.opts.session,
                title: self.state.focused_title(),
                prefix: &self.opts.prefix_label,
                pending: self.prefix_pending,
                message: self.notice.as_ref().map(|(text, _)| text.as_str()),
            },
            selection: self.selection_span(),
        };
        self.ui.draw(|frame| view.draw(frame)).map_err(io)?;
        if self.host.sync_output {
            execute!(self.ui.backend_mut(), EndSynchronizedUpdate).map_err(io)?;
        }
        self.dirty = false;
        self.last_draw = Instant::now();
        Ok(())
    }

    fn finish(&mut self) -> Result<(), TuiError> {
        self.mirror.leave(&mut self.out);
        let out = std::mem::take(&mut self.out);
        write_host(&mut self.ui, &out)
    }
}

/// Tells the server what the host can do and how big it is.
fn introduce(
    ui: &DefaultTerminal,
    link: &ServerLink,
    host: HostInfo,
) -> Result<ScreenSize, TuiError> {
    let area = ui
        .size()
        .map_err(|e| TuiError::io("reading terminal size", e))?;
    let size = ScreenSize::new(area.width, area.height);
    link.send(&ClientMsg::HostInfo(host))?;
    link.send(&ClientMsg::Resize(AttachView::pane_area(size)))?;
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
        tx.send(ClientEvent::Command(b'x')).expect("send");
        tx.send(ClientEvent::Host(HostInfo::default()))
            .expect("send");
        let (_, backlog) = wait_for_host(&rx);
        assert_eq!(
            Vec::from(backlog),
            [
                ClientEvent::Server(ServerMsg::Pong(1)),
                ClientEvent::Command(b'x')
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
