//! M1 spike 1 prototype: one pane, no chrome, everything in one process.
//!
//! portable-pty → `Terminal` (OscTap + alacritty) → `PaneView` → ratatui.
//! Run with `cargo run -p monarch-tui --example single_pane [-- cmd args…]`
//! and try vim, htop, less, claude, codex. `Ctrl-b q` quits.
//!
//! Not yet mirrored to the host (spike 3): kitty keyboard, mouse, bracketed
//! paste, focus events, host colours.

use std::io::{Read, Write};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use monarch_proto::ScreenSize;
use monarch_term::{EngineEvent, Terminal};
use monarch_tui::{InputEvent, PaneView, PrefixParser};
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;

const FRAME: Duration = Duration::from_millis(16);
const PREFIX: u8 = 0x02; // Ctrl-b

enum Msg {
    Output,
    Quit,
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
    let _child = pty.slave.spawn_command(command())?;
    drop(pty.slave);

    let term = Arc::new(Mutex::new(Terminal::new(size, 10_000)));
    let writer: SharedWriter = Arc::new(Mutex::new(pty.master.take_writer()?));
    let (tx, rx) = mpsc::channel();
    spawn_reader(
        pty.master.try_clone_reader()?,
        Arc::clone(&term),
        Arc::clone(&writer),
        tx.clone(),
    );
    spawn_stdin(Arc::clone(&writer), tx);

    let mut last_draw = Instant::now() - FRAME;
    let mut dirty = true;
    loop {
        match rx.recv_timeout(FRAME) {
            Ok(Msg::Output) => dirty = true,
            Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => return Ok(()),
            Err(RecvTimeoutError::Timeout) => {}
        }
        size = handle_resize(ui, size, &term, pty.master.as_ref())?.unwrap_or(size);
        if dirty && last_draw.elapsed() >= FRAME {
            draw(ui, &term)?;
            last_draw = Instant::now();
            dirty = false;
        }
    }
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
fn spawn_reader(
    mut pty: Box<dyn Read + Send>,
    term: Arc<Mutex<Terminal>>,
    writer: SharedWriter,
    tx: Sender<Msg>,
) {
    thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        let mut events = Vec::new();
        while let Ok(n @ 1..) = pty.read(&mut buf) {
            term.lock().expect("term lock").feed(&buf[..n], &mut events);
            for event in events.drain(..) {
                if let EngineEvent::Reply(bytes) = event {
                    let _ = writer.lock().expect("writer lock").write_all(&bytes);
                }
            }
            if tx.send(Msg::Output).is_err() {
                return;
            }
        }
        let _ = tx.send(Msg::Quit);
    });
}

/// Raw stdin → PTY, byte-exact; `Ctrl-b q` quits, other commands are ignored.
fn spawn_stdin(writer: SharedWriter, tx: Sender<Msg>) {
    thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut buf = [0u8; 4096];
        let mut parser = PrefixParser::new(PREFIX);
        let mut events = Vec::new();
        while let Ok(n @ 1..) = stdin.read(&mut buf) {
            parser.feed(&buf[..n], &mut events);
            for event in events.drain(..) {
                match event {
                    InputEvent::Forward(bytes) => {
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

fn draw(ui: &mut ratatui::DefaultTerminal, term: &Mutex<Terminal>) -> Result<()> {
    let screen = term.lock().expect("term lock").screen();
    ui.draw(|frame| {
        let view = PaneView::new(&screen);
        if let Some(pos) = view.cursor_position(frame.area()) {
            frame.set_cursor_position(pos);
        }
        frame.render_widget(view, frame.area());
    })?;
    Ok(())
}
