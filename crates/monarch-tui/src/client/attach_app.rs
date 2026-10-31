//! The attached client's main loop: apply frames, mirror modes, draw.

use std::collections::VecDeque;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use monarch_core::Key;
use monarch_proto::{ClientMsg, HostInfo, ScreenSize};
use ratatui::DefaultTerminal;

use super::{
    Applied, AttachOptions, AttachOutcome, AttachState, ClientEvent, ConfigWatch, CopyMode,
    DragScroll, InputKeys, Selection, ServerLink, SessionLister, SharedFocus, SidebarScroll,
};
use crate::{HostModeMirror, TuiError, WhichKeyEntry};

mod animate;
mod copy;
mod draw;
mod help;
mod inbox;
mod menu;
mod picker;
mod popups;
mod projects;
mod reload;
mod rename;
mod review;
mod select;
mod sessions;
mod sidebar;
mod startup;
mod welcome;
mod which_key;
mod worktree;

use popups::Popups;
use startup::{Started, warnings_notice, write_host};

/// Minimum time between frames drawn to the host.
const FRAME: Duration = Duration::from_millis(16);
/// How long config warnings stay in the status line.
const WARNING: Duration = Duration::from_secs(8);

/// A running attach session.
pub struct AttachApp {
    ui: DefaultTerminal,
    link: ServerLink,
    events: Receiver<ClientEvent>,
    /// For results worked out off this thread (the session list).
    post: mpsc::Sender<ClientEvent>,
    /// Finds the running sessions for the session picker.
    lister: Arc<dyn SessionLister>,
    backlog: VecDeque<ClientEvent>,
    state: AttachState,
    mirror: HostModeMirror,
    host: HostInfo,
    focus: Arc<SharedFocus>,
    /// New keys for the input thread after a reload.
    rekey: mpsc::Sender<InputKeys>,
    /// Set when this attach ends, so its input thread lets go of the
    /// terminal.
    input_stop: Arc<AtomicBool>,
    opts: AttachOptions,
    /// The config file, re-read when it changes.
    config: ConfigWatch,
    prefix_pending: bool,
    /// When the prefix was pressed, while it waits for a key.
    prefix_at: Option<Instant>,
    /// The key popup is up.
    popup: bool,
    which_key: Vec<WhichKeyEntry>,
    /// The pop-ups open over the panes.
    popups: Popups,
    /// Copy mode on the focused pane, which takes the keys.
    copy: Option<CopyMode>,
    selection: Option<Selection>,
    /// Scrolling while a selection drag is past the pane's edge.
    drag: Option<DragScroll>,
    /// The sidebar is wanted (it still hides on narrow hosts).
    sidebar: bool,
    sidebar_scroll: SidebarScroll,
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
    pub fn attach(
        opts: &AttachOptions,
        lister: Arc<dyn SessionLister>,
    ) -> Result<AttachOutcome, TuiError> {
        let (link, reader) = ServerLink::connect(&opts.socket)?;
        let ui = ratatui::init();
        let result = Self::start(ui, link, reader, opts, lister).and_then(|mut app| {
            let outcome = app.run();
            app.finish()?;
            outcome
        });
        ratatui::restore();
        result
    }

    fn start(
        ui: DefaultTerminal,
        link: ServerLink,
        reader: UnixStream,
        opts: &AttachOptions,
        lister: Arc<dyn SessionLister>,
    ) -> Result<Self, TuiError> {
        let s = Started::new(ui, link, reader, opts)?;
        let mut app = Self {
            ui: s.ui,
            link: s.link,
            events: s.events,
            post: s.post,
            lister,
            backlog: s.backlog,
            state: s.state,
            mirror: s.mirror,
            host: s.host,
            focus: s.focus,
            rekey: s.rekey,
            input_stop: s.input_stop,
            opts: opts.clone(),
            config: ConfigWatch::new(opts.config_path.clone(), Instant::now()),
            prefix_pending: false,
            prefix_at: None,
            popup: false,
            which_key: WhichKeyEntry::from_keymap(&opts.keymap),
            popups: Popups::default(),
            copy: None,
            selection: None,
            drag: None,
            sidebar: opts.sidebar,
            sidebar_scroll: SidebarScroll::default(),
            passthrough: false,
            glyphs: animate::Glyphs::new(s.host.background, &opts.theme),
            notice: warnings_notice(opts),
            out: s.out,
            size: s.size,
            dirty: true,
            last_draw: Instant::now() - FRAME,
        };
        if opts.welcome {
            app.open_welcome();
        }
        Ok(app)
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
            ClientEvent::SidebarScroll(lines) => self.scroll_sidebar(lines),
            ClientEvent::Sessions(list) => self.loaded_sessions(list),
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
        for (pane, review) in applied.reviews {
            self.loaded_review(pane, review);
        }
        for made in applied.worktrees {
            self.worktree_made(made);
        }
        for list in applied.worktree_lists {
            self.loaded_worktrees(list);
        }
        for removed in applied.removed {
            self.worktree_removed(removed);
        }
        if applied.redraw && self.drag.is_some() {
            self.drag_extend();
        }
        if applied.redraw {
            self.refresh_inbox();
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

    /// Gives the terminal back as it was, lets go of its input and closes
    /// the connection (another attach may follow in this process).
    fn finish(&mut self) -> Result<(), TuiError> {
        self.input_stop.store(true, Ordering::SeqCst);
        self.link.close();
        self.mirror.leave(&mut self.out);
        let out = std::mem::take(&mut self.out);
        write_host(&mut self.ui, &out)
    }
}
