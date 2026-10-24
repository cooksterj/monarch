//! M1 spike prototype: one pane, no chrome, everything in one process.
//!
//! portable-pty → `Terminal` (OscTap + alacritty) → `PaneView` → ratatui.
//! Run with `cargo run -p monarch-mux-tui --example single_pane [-- cmd args…]`
//! and try vim, htop, less, claude, codex. `Ctrl-b q` quits.
//!
//! Spike 3: the host is probed at startup (`HostProbe`), the pane's modes are
//! mirrored onto it (`HostModeMirror`), and host-only input (paste markers,
//! focus events) is filtered per pane (`HostInputFilter`). Mouse coordinates
//! are passed through unchanged, which is only right for a single
//! full-screen pane (re-encoding is spike 5).

use std::io::{Read, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use monarch_proto::{HostInfo, ModeFlags, ScreenSize};
use monarch_term::{EngineEvent, HostProfile, Terminal};
use monarch_tui::{HostInputFilter, HostModeMirror, HostProbe, InputEvent, PaneView, PrefixParser};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, SlavePty, native_pty_system};
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use ratatui::crossterm::{execute, queue};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

const FRAME: Duration = Duration::from_millis(16);
const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
const PREFIX: u8 = 0x02; // Ctrl-b

enum Msg {
    Output,
    Title(Option<String>),
    Bell,
    Host(HostInfo),
    Quit,
}

/// The pane's terminal plus what the input thread needs to know about it.
struct Pane {
    term: Arc<Mutex<Terminal>>,
    writer: SharedWriter,
    modes: Arc<AtomicU32>,
}

impl Pane {
    fn new(size: ScreenSize, master: &dyn MasterPty) -> Result<Self> {
        Ok(Self {
            term: Arc::new(Mutex::new(Terminal::new(size, 10_000))),
            writer: Arc::new(Mutex::new(master.take_writer()?)),
            modes: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Starts the pane's program once the host is known, so its first
    /// colour queries are answered with the host's colours.
    fn spawn(&self, slave: Box<dyn SlavePty + Send>, host: &HostInfo) -> Result<Box<dyn Child>> {
        let profile = HostProfile::from(host);
        self.term
            .lock()
            .expect("term lock")
            .set_host_profile(profile);
        let child = slave.spawn_command(command())?;
        // Only the child may hold the slave, or EOF never reaches the reader.
        drop(slave);
        Ok(child)
    }
}

fn main() -> Result<()> {
    let mut ui = ratatui::init();
    let result = run(&mut ui);
    ratatui::restore();
    result
}

fn run(ui: &mut ratatui::DefaultTerminal) -> Result<()> {
    let area = ui.size()?;
    let mut size = ScreenSize::new(area.width, area.height);
    let pty = native_pty_system().openpty(pty_size(size))?;
    let pane = Pane::new(size, pty.master.as_ref())?;
    let (tx, rx) = mpsc::channel();
    spawn_reader(pty.master.try_clone_reader()?, &pane, tx.clone());
    write_host(ui, HostProbe::QUERY)?;
    spawn_stdin(&pane, tx);

    let host = wait_for_host(&rx);
    let _child = pane.spawn(pty.slave, &host)?;
    let mut mirror = HostModeMirror::new(&host);
    let mut out = Vec::new();
    mirror.enter(&mut out);

    let mut last_draw = Instant::now() - FRAME;
    let mut dirty = true;
    loop {
        match rx.recv_timeout(FRAME) {
            Ok(Msg::Output | Msg::Host(_)) => dirty = true,
            Ok(Msg::Title(title)) => mirror.sync_title(title.as_deref(), &mut out),
            Ok(Msg::Bell) => out.push(0x07),
            Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        size = handle_resize(ui, size, &pane.term, pty.master.as_ref())?.unwrap_or(size);
        if dirty && last_draw.elapsed() >= FRAME {
            draw(ui, &pane.term, &mut mirror, &mut out, host.sync_output)?;
            last_draw = Instant::now();
            dirty = false;
        }
    }
    mirror.leave(&mut out);
    write_host(ui, &out)
}

/// Waits for the probe answers; a host that never answers gets defaults.
fn wait_for_host(rx: &Receiver<Msg>) -> HostInfo {
    let deadline = Instant::now() + PROBE_TIMEOUT;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(Msg::Host(info)) => return info,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    HostInfo::default()
}

fn write_host(ui: &mut ratatui::DefaultTerminal, bytes: &[u8]) -> Result<()> {
    let backend = ui.backend_mut();
    backend.write_all(bytes)?;
    backend.flush()?;
    Ok(())
}

fn command() -> CommandBuilder {
    let mut args = std::env::args().skip(1);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut cmd = CommandBuilder::new(args.next().unwrap_or(shell));
    cmd.args(args);
    if let Ok(cwd) = std::env::current_dir() {
        cmd.cwd(cwd);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "monarch");
    cmd.env_remove("TMUX");
    cmd
}

fn pty_size(size: ScreenSize) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

/// PTY → OscTap/engine; query replies go straight back to the PTY so they
/// stay in order with the output that asked for them.
fn spawn_reader(mut pty: Box<dyn Read + Send>, pane: &Pane, tx: Sender<Msg>) {
    let (term, writer, modes) = (
        Arc::clone(&pane.term),
        Arc::clone(&pane.writer),
        Arc::clone(&pane.modes),
    );
    thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        let mut events = Vec::new();
        while let Ok(n @ 1..) = pty.read(&mut buf) {
            {
                let mut term = term.lock().expect("term lock");
                term.feed(&buf[..n], &mut events);
                modes.store(term.modes().flags.bits(), Ordering::Relaxed);
            }
            for event in events.drain(..) {
                let msg = match event {
                    EngineEvent::Reply(bytes) => {
                        let _ = writer.lock().expect("writer lock").write_all(&bytes);
                        continue;
                    }
                    EngineEvent::Title(title) => Msg::Title(title),
                    EngineEvent::Bell => Msg::Bell,
                    _ => continue,
                };
                let _ = tx.send(msg);
            }
            if tx.send(Msg::Output).is_err() {
                return;
            }
        }
        let _ = tx.send(Msg::Quit);
    });
}

/// Raw stdin → probe → prefix parser → per-pane filter → PTY.
/// `Ctrl-b q` quits; other commands are ignored.
fn spawn_stdin(pane: &Pane, tx: Sender<Msg>) {
    let (writer, modes) = (Arc::clone(&pane.writer), Arc::clone(&pane.modes));
    thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 4096];
        let mut probe = HostProbe::new();
        let deadline = Instant::now() + PROBE_TIMEOUT;
        let mut parser = PrefixParser::new(PREFIX);
        let mut filter = HostInputFilter::new();
        let (mut input, mut events, mut bytes) = (Vec::new(), Vec::new(), Vec::new());
        while let Ok(n @ 1..) = stdin.read(&mut buf) {
            input.clear();
            let probing = !probe.is_done();
            if probe.feed(&buf[..n], &mut input) && probing {
                let _ = tx.send(Msg::Host(probe.info()));
            } else if !probe.is_done() && Instant::now() > deadline {
                probe.abandon(&mut input);
            }
            parser.feed(&input, &mut events);
            for event in events.drain(..) {
                match event {
                    InputEvent::Forward(raw) => {
                        bytes.clear();
                        let flags = ModeFlags::from_bits_truncate(modes.load(Ordering::Relaxed));
                        filter.filter(&raw, flags, &mut bytes);
                        let _ = writer.lock().expect("writer lock").write_all(&bytes);
                    }
                    InputEvent::Command(b'q') => {
                        let _ = tx.send(Msg::Quit);
                        return;
                    }
                    InputEvent::Command(_) => {}
                }
            }
        }
    });
}

fn handle_resize(
    ui: &mut ratatui::DefaultTerminal,
    current: ScreenSize,
    term: &Mutex<Terminal>,
    master: &dyn MasterPty,
) -> Result<Option<ScreenSize>> {
    let area = ui.size()?;
    let size = ScreenSize::new(area.width, area.height);
    if size == current {
        return Ok(None);
    }
    term.lock().expect("term lock").resize(size);
    master.resize(pty_size(size))?;
    ui.autoresize()?;
    Ok(Some(size))
}

/// Mirrors the pane's modes onto the host, then draws, inside one
/// synchronized update when the host supports it.
fn draw(
    ui: &mut ratatui::DefaultTerminal,
    term: &Mutex<Terminal>,
    mirror: &mut HostModeMirror,
    out: &mut Vec<u8>,
    sync_output: bool,
) -> Result<()> {
    let screen = term.lock().expect("term lock").screen();
    mirror.sync(&screen.modes, &screen.cursor, out);
    if sync_output {
        queue!(ui.backend_mut(), BeginSynchronizedUpdate)?;
    }
    ui.backend_mut().write_all(out)?;
    out.clear();
    ui.draw(|frame| {
        let view = PaneView::new(&screen);
        if let Some(pos) = view.cursor_position(frame.area()) {
            frame.set_cursor_position(pos);
        }
        frame.render_widget(view, frame.area());
    })?;
    if sync_output {
        execute!(ui.backend_mut(), EndSynchronizedUpdate)?;
    }
    Ok(())
}
