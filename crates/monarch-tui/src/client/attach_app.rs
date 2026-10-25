//! The attached client's main loop: apply frames, mirror modes, draw.

use std::collections::VecDeque;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use monarch_core::Key;
use monarch_proto::{ClientMsg, HostInfo, ScreenSize};
use ratatui::DefaultTerminal;

use super::{
    Applied, AttachOptions, AttachOutcome, AttachState, ClientEvent, ConfigWatch, ContextMenu,
    CopyMode, DragScroll, InputKeys, Selection, ServerLink, SharedFocus,
};
use crate::{HostModeMirror, HostProbe, Picker, TuiError, WhichKeyEntry};

mod animate;
mod copy;
mod draw;
mod menu;
mod picker;
mod reload;
mod select;
mod sidebar;
mod startup;
mod which_key;

use startup::{enter_host, introduce, spawn_input, wait_for_host, warnings_notice, write_host};

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
    /// New keys for the input thread after a reload.
    rekey: mpsc::Sender<InputKeys>,
    opts: AttachOptions,
    /// The config file, re-read when it changes.
    config: ConfigWatch,
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
    /// The right-click menu, which takes clicks and keys.
    menu: Option<ContextMenu>,
    selection: Option<Selection>,
    /// Scrolling while a selection drag is past the pane's edge.
    drag: Option<DragScroll>,
    /// The sidebar is wanted (it still hides on narrow hosts).
    sidebar: bool,
    passthrough: bool,
    /// Agent glyph animation state, its colours and the next frame.
    glyphs: animate::Glyphs,
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
        let rekey = spawn_input(&link, opts, Arc::clone(&focus), tx);
        let (host, backlog) = wait_for_host(&events);
        let size = introduce(&ui, &link, host, opts)?;
        let (mirror, out) = enter_host(&host, opts);
        let mut state = AttachState::new();
        state.set_left(sidebar::sidebar_cols(opts, opts.sidebar, size));
        Ok(Self {
            ui,
            link,
            events,
            backlog,
            state,
            mirror,
            host,
            focus,
            rekey,
            opts: opts.clone(),
            config: ConfigWatch::new(opts.config_path.clone(), Instant::now()),
            prefix_pending: false,
            prefix_at: None,
            popup: false,
            which_key: WhichKeyEntry::from_keymap(&opts.keymap),
            picker: None,
            copy: None,
            menu: None,
            selection: None,
            drag: None,
            sidebar: opts.sidebar,
            passthrough: false,
            glyphs: animate::Glyphs::new(host.background),
            notice: warnings_notice(opts),
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
            self.animate();
            self.watch_config();
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
            ClientEvent::Menu { col, row } => self.open_menu(col, row),
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
