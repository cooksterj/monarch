//! A pane backed by a real PTY and child process.

mod reader_thread;
mod spawn_params;
mod writer_thread;

use std::sync::Arc;
use std::sync::mpsc::{self, Sender};

use monarch_proto::ScreenSize;
use monarch_term::Terminal;
use portable_pty::{ChildKiller, MasterPty, PtySize, native_pty_system};

use crate::{BoxedError, PaneIo, PaneShared, ServerError};
use reader_thread::{ReaderParams, ReaderThread};
pub use spawn_params::SpawnParams;
use writer_thread::WriterThread;

/// A running pane: two std threads (PTY reader, PTY writer) around a
/// shared emulator. Dropping it kills the child.
pub struct PaneRuntime {
    shared: Arc<PaneShared>,
    input: Sender<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    pid: Option<u32>,
}

impl PaneRuntime {
    /// Opens a PTY, starts the program and its reader/writer threads.
    ///
    /// # Errors
    /// The PTY cannot be opened, the program cannot start, or a thread
    /// cannot be spawned.
    pub fn spawn(p: SpawnParams<'_>) -> Result<Self, ServerError> {
        let pair = native_pty_system()
            .openpty(pty_size(p.size))
            .map_err(pty_error)?;
        let mut term = Terminal::new(p.size, p.scrollback);
        term.set_host_profile(p.profile);
        if let Some(banner) = p.banner {
            term.feed(banner.as_bytes(), &mut Vec::new());
        }
        let child = pair
            .slave
            .spawn_command(p.spec.command(p.id, p.env))
            .map_err(pty_error)?;
        // Only the child may hold the slave, or EOF never reaches the reader.
        drop(pair.slave);
        let killer = child.clone_killer();
        let pid = child.process_id();
        let shared = Arc::new(PaneShared::new(term));
        let (input, rx) = mpsc::channel();
        let thread = |e| ServerError::io("starting pane thread", e);
        WriterThread::spawn(p.id, pair.master.take_writer().map_err(pty_error)?, rx)
            .map_err(thread)?;
        ReaderThread::spawn(ReaderParams {
            pane: p.id,
            reader: pair.master.try_clone_reader().map_err(pty_error)?,
            child,
            shared: Arc::clone(&shared),
            replies: input.clone(),
            events: p.events,
            wake: p.wake,
        })
        .map_err(thread)?;
        Ok(Self {
            shared,
            input,
            master: pair.master,
            killer,
            pid,
        })
    }
}

impl PaneIo for PaneRuntime {
    fn shared(&self) -> &PaneShared {
        &self.shared
    }

    fn send(&self, bytes: Vec<u8>) {
        // A closed channel means the program exited; the exit event follows.
        let _ = self.input.send(bytes);
    }

    fn resize(&self, size: ScreenSize) {
        self.shared.lock().resize(size);
        let _ = self.master.resize(pty_size(size));
    }

    fn process_id(&self) -> Option<u32> {
        self.pid
    }
}

impl Drop for PaneRuntime {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}

/// portable-pty reports failures as an opaque boxed error.
fn pty_error(e: impl Into<BoxedError>) -> ServerError {
    ServerError::Pty(e.into())
}

fn pty_size(size: ScreenSize) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pty_size_carries_cells_only() {
        let size = pty_size(ScreenSize::new(120, 40));
        assert_eq!((size.cols, size.rows, size.pixel_width), (120, 40, 0));
    }
}
