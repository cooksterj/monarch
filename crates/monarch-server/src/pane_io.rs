//! The session's view of a pane: its emulator plus a way to reach the PTY.

use monarch_proto::ScreenSize;

use crate::PaneShared;

/// What the session needs from a pane. [`crate::PaneRuntime`] implements it
/// over a real PTY; tests implement it over a bare terminal, so session and
/// rendering logic run without spawning processes.
pub trait PaneIo: Send {
    /// The emulator and dirty flag.
    fn shared(&self) -> &PaneShared;

    /// Queues bytes for the program's input, in order with query replies.
    fn send(&self, bytes: Vec<u8>);

    /// Resizes the pane. The default resizes only the emulator; PTY-backed
    /// panes must resize the emulator first, then the PTY, so the program's
    /// redraw after SIGWINCH lands on a grid of the new size.
    fn resize(&self, size: ScreenSize) {
        self.shared().lock().resize(size);
    }

    /// Hangs up the program (closing its pane); it exits in its own time
    /// and the exit removes the pane.
    fn close(&self) {}

    /// The program's process id, when it runs as a real process.
    fn process_id(&self) -> Option<u32> {
        None
    }

    /// The process in the terminal's foreground (`vim` started from the
    /// shell); by default the program itself.
    fn foreground_process(&self) -> Option<u32> {
        self.process_id()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use monarch_term::Terminal;

    use super::*;

    struct Fake {
        shared: PaneShared,
        sent: Mutex<Vec<u8>>,
    }

    impl PaneIo for Fake {
        fn shared(&self) -> &PaneShared {
            &self.shared
        }
        fn send(&self, bytes: Vec<u8>) {
            self.sent.lock().expect("lock").extend(bytes);
        }
    }

    #[test]
    fn default_resize_resizes_the_emulator() {
        let fake = Fake {
            shared: PaneShared::new(Terminal::new(ScreenSize::new(10, 2), 0)),
            sent: Mutex::new(Vec::new()),
        };
        fake.resize(ScreenSize::new(30, 5));
        assert_eq!(fake.shared().lock().size(), ScreenSize::new(30, 5));
        fake.send(b"x".to_vec());
        assert_eq!(*fake.sent.lock().expect("lock"), b"x");
        assert_eq!(fake.process_id(), None, "no process by default");
        assert_eq!(fake.foreground_process(), None);
    }
}
