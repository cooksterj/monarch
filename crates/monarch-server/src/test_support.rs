//! Test doubles shared by the crate's unit tests.

use std::sync::Mutex;

use monarch_proto::ScreenSize;
use monarch_term::Terminal;

use crate::{PaneIo, PaneShared};

/// A pane with an emulator but no PTY; records what it was sent.
pub struct FakePane {
    shared: PaneShared,
    /// Bytes passed to [`PaneIo::send`].
    pub sent: Mutex<Vec<u8>>,
}

impl FakePane {
    /// A boxed fake of the given size.
    pub fn boxed(size: ScreenSize) -> Box<dyn PaneIo> {
        Box::new(Self {
            shared: PaneShared::new(Terminal::new(size, 100)),
            sent: Mutex::new(Vec::new()),
        })
    }
}

impl PaneIo for FakePane {
    fn shared(&self) -> &PaneShared {
        &self.shared
    }
    fn send(&self, bytes: Vec<u8>) {
        self.sent.lock().expect("lock").extend(bytes);
    }
}
