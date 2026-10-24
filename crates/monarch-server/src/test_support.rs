//! Test doubles shared by the crate's unit tests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use monarch_proto::ScreenSize;
use monarch_term::Terminal;
use tokio::sync::mpsc;

use crate::{PaneEnv, PaneIo, PaneShared, ServerContext};

/// A pane with an emulator but no PTY; records what it was sent.
pub struct FakePane {
    shared: PaneShared,
    pid: Option<u32>,
    closed: Arc<AtomicBool>,
    /// Bytes passed to [`PaneIo::send`].
    pub sent: Mutex<Vec<u8>>,
}

impl FakePane {
    /// A boxed fake of the given size.
    pub fn boxed(size: ScreenSize) -> Box<dyn PaneIo> {
        Box::new(Self::sized(size, None))
    }

    /// A boxed 80×24 fake with no process (like a pane in a test).
    pub fn shell() -> Box<dyn PaneIo> {
        Self::boxed(ScreenSize::new(80, 24))
    }

    /// A boxed 80×24 fake whose process is `pid`.
    pub fn with_pid(pid: u32) -> Box<dyn PaneIo> {
        Box::new(Self::sized(ScreenSize::new(80, 24), Some(pid)))
    }

    /// A boxed 80×24 fake and a flag set when it is closed.
    pub fn tracked() -> (Box<dyn PaneIo>, Arc<AtomicBool>) {
        let fake = Self::sized(ScreenSize::new(80, 24), None);
        let closed = Arc::clone(&fake.closed);
        (Box::new(fake), closed)
    }

    fn sized(size: ScreenSize, pid: Option<u32>) -> Self {
        Self {
            shared: PaneShared::new(Terminal::new(size, 100)),
            pid,
            closed: Arc::new(AtomicBool::new(false)),
            sent: Mutex::new(Vec::new()),
        }
    }
}

impl PaneIo for FakePane {
    fn shared(&self) -> &PaneShared {
        &self.shared
    }
    fn send(&self, bytes: Vec<u8>) {
        self.sent.lock().expect("lock").extend(bytes);
    }
    fn process_id(&self) -> Option<u32> {
        self.pid
    }
    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }
}

/// A server context with no panes whose events go nowhere.
pub fn context() -> ServerContext {
    let (events, _rx) = mpsc::unbounded_channel();
    let env = PaneEnv {
        session: "test".into(),
        socket: "/tmp/test.sock".into(),
    };
    ServerContext::new(env, events, 100)
}
