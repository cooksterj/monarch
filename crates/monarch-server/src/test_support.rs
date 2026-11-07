//! Test doubles shared by the crate's unit tests.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use monarch_core::{Clock, UnixMillis};
use monarch_proto::ScreenSize;
use monarch_term::Terminal;
use tokio::sync::mpsc;

use crate::{LaunchConfig, PaneEnv, PaneIo, PaneShared, ServerContext};

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
    let env = PaneEnv::new("test", "/tmp/test.sock");
    let launch = LaunchConfig {
        scrollback: 100,
        agent_scrollback: 100,
        ..LaunchConfig::default()
    };
    ServerContext::new(env, events, launch)
}

/// A clock that stands still until a test moves it.
#[derive(Debug, Default)]
pub struct FakeClock(AtomicU64);

impl FakeClock {
    /// A shared clock reading `now`.
    pub fn at(now: u64) -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(now)))
    }

    /// Sets the time.
    pub fn set(&self, now: u64) {
        self.0.store(now, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> UnixMillis {
        UnixMillis(self.0.load(Ordering::SeqCst))
    }
}
