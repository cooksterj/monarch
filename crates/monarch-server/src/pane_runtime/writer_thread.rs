//! Writes queued input and query replies to the PTY; one std thread per pane.

use std::io::Write;
use std::sync::mpsc::Receiver;
use std::thread::{self, JoinHandle};

use monarch_core::PaneId;

/// Blocking writes from one ordered channel, so user input and terminal
/// query replies reach the program in the order they were produced. Ends
/// when every sender is gone or the PTY stops accepting writes.
pub(super) struct WriterThread;

impl WriterThread {
    pub(super) fn spawn(
        pane: PaneId,
        mut writer: Box<dyn Write + Send>,
        rx: Receiver<Vec<u8>>,
    ) -> std::io::Result<JoinHandle<()>> {
        thread::Builder::new()
            .name(format!("monarch-write-{pane}"))
            .spawn(move || {
                while let Ok(bytes) = rx.recv() {
                    if writer
                        .write_all(&bytes)
                        .and_then(|()| writer.flush())
                        .is_err()
                    {
                        return;
                    }
                }
            })
    }
}
