//! An opt-in trace of how mouse reports are routed, for diagnosing mouse
//! trouble on a user's terminal.
//!
//! Set `MONARCH_DEBUG_MOUSE=<file>` to append one line per decoded mouse
//! report: the event, the focused pane's place and what was done with it.
//! Keystrokes are never written; input swallowed by a pop-up is logged by
//! length only.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::Instant;

use crate::MouseEvent;
use crate::client::{FocusView, MouseAction};

/// The environment variable naming the log file.
const VAR: &str = "MONARCH_DEBUG_MOUSE";

/// Where the trace goes; does nothing unless enabled.
pub(super) struct MouseLog {
    out: Option<Box<dyn Write + Send>>,
    start: Instant,
}

impl Default for MouseLog {
    fn default() -> Self {
        Self {
            out: None,
            start: Instant::now(),
        }
    }
}

impl MouseLog {
    /// Opens the file `MONARCH_DEBUG_MOUSE` names, if set.
    pub(super) fn from_env() -> Self {
        let file = std::env::var_os(VAR).and_then(|path| open(&path));
        Self {
            out: file.map(|f| Box::new(f) as Box<dyn Write + Send>),
            ..Self::default()
        }
    }

    /// A log writing to `out`, for tests.
    #[cfg(test)]
    fn to(out: impl Write + Send + 'static) -> Self {
        Self {
            out: Some(Box::new(out)),
            ..Self::default()
        }
    }

    /// A report and what the router made of it.
    pub(super) fn routed(&mut self, event: &MouseEvent, view: &FocusView, action: &MouseAction) {
        self.line(format_args!(
            "{event:?} focus={} -> {action:?}",
            place(view)
        ));
    }

    /// A press that focused another pane.
    pub(super) fn focused(
        &mut self,
        event: &MouseEvent,
        view: &FocusView,
        pane: impl std::fmt::Debug,
    ) {
        self.line(format_args!(
            "{event:?} focus={} -> focus {pane:?}",
            place(view)
        ));
    }

    /// A report dropped while a click-to-focus is held.
    pub(super) fn swallowed(&mut self, event: &MouseEvent) {
        self.line(format_args!("{event:?} -> dropped (focusing)"));
    }

    /// Keys that look like a mouse report the extractor didn't decode,
    /// by length only; ordinary keys are not logged.
    pub(super) fn keys(&mut self, keys: &[u8]) {
        let mouse_like = keys.windows(3).any(|w| w == b"\x1b[M" || w == b"\x1b[<");
        if mouse_like {
            self.line(format_args!(
                "{} key bytes with a mouse prefix -> pane",
                keys.len()
            ));
        }
    }

    /// Bytes as read from the terminal: control sequences shown, typed
    /// text (and the key after a lone `ESC`) replaced by a count.
    pub(super) fn raw(&mut self, bytes: &[u8]) {
        if self.out.is_some() {
            self.line(format_args!("read {}: {}", bytes.len(), redact(bytes)));
        }
    }

    /// Input handed to an open pop-up, by length only.
    pub(super) fn overlay(&mut self, len: usize) {
        self.line(format_args!("{len} bytes -> pop-up"));
    }

    fn line(&mut self, what: std::fmt::Arguments<'_>) {
        let Some(out) = self.out.as_mut() else {
            return;
        };
        let ms = self.start.elapsed().as_millis();
        if writeln!(out, "{ms:>8} {what}")
            .and_then(|()| out.flush())
            .is_err()
        {
            self.out = None;
        }
    }
}

/// `bytes` with each escape sequence spelled out and every run of other
/// bytes shown only as its length, e.g. `\e[<0;5;3M ‹2›`.
fn redact(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut i, mut text) = (0, 0);
    while i < bytes.len() {
        let byte = bytes[i];
        i += 1;
        if byte != 0x1b && byte != 0x02 {
            text += 1;
            continue;
        }
        if text > 0 {
            out.push_str(&format!("‹{text}› "));
            text = 0;
        }
        if byte == 0x02 {
            out.push_str("^B ");
            continue;
        }
        let end = sequence_end(bytes, i);
        out.push_str("\\e");
        match end {
            Some(end) => {
                out.extend(bytes[i..end].iter().map(|&b| shown(b)));
                i = end;
            }
            None if i < bytes.len() => {
                out.push_str("‹alt›");
                i += 1;
            }
            None => {}
        }
        out.push(' ');
    }
    if text > 0 {
        out.push_str(&format!("‹{text}›"));
    }
    out
}

/// Where the CSI (`[`) or SS3 (`O`) sequence starting at `start` ends.
fn sequence_end(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'O' => Some((start + 2).min(bytes.len())),
        b'[' => {
            let body = &bytes[start + 1..];
            let last = body.iter().position(|b| (0x40..=0x7e).contains(b));
            Some(last.map_or(bytes.len(), |n| start + 2 + n))
        }
        _ => None,
    }
}

fn shown(byte: u8) -> char {
    if byte.is_ascii_graphic() {
        char::from(byte)
    } else {
        '?'
    }
}

fn open(path: &std::ffi::OsStr) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(path).ok()
}

/// The focused pane and its host rectangle.
fn place(view: &FocusView) -> String {
    format!(
        "{:?}@{},{} {}x{} {:?}",
        view.pane, view.x, view.y, view.cols, view.rows, view.modes
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use monarch_core::PaneId;

    use super::*;
    use crate::{MouseButton, MouseKind};

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("sink").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn lines_name_the_event_focus_and_action() {
        let sink = Sink::default();
        let mut log = MouseLog::to(sink.clone());
        let event = MouseEvent {
            kind: MouseKind::Press(MouseButton::Left),
            col: 3,
            row: 4,
            mods: 0,
        };
        let view = FocusView {
            pane: Some(PaneId(2)),
            x: 1,
            y: 1,
            cols: 10,
            rows: 5,
            ..FocusView::default()
        };
        log.routed(&event, &view, &MouseAction::Ignore);
        log.overlay(7);
        log.keys(b"ls\r");
        log.keys(b"\x1b[M !!");
        let text = String::from_utf8(sink.0.lock().expect("sink").clone()).expect("utf8");
        assert!(text.contains("focus=Some(PaneId(2))@1,1 10x5"), "{text}");
        assert!(text.contains("-> Ignore"), "{text}");
        assert!(text.contains("7 bytes -> pop-up"), "{text}");
        assert!(!text.contains("3 key bytes"), "plain keys stay private");
        assert!(text.contains("6 key bytes with a mouse prefix"), "{text}");
    }

    #[test]
    fn raw_input_shows_sequences_but_not_text() {
        assert_eq!(
            redact(b"\x1b[<0;5;3Mhi\x02%\x1bx\x1bOA"),
            "\\e[<0;5;3M ‹2› ^B ‹1› \\e‹alt› \\eOA "
        );
        assert_eq!(redact(b"secret"), "‹6›");
        assert_eq!(redact(b"\x1b[1"), "\\e[1 ", "cut short");
        assert_eq!(redact(b"\x1b"), "\\e ");
    }

    #[test]
    fn disabled_logs_write_nothing() {
        let mut log = MouseLog::default();
        log.overlay(1);
        assert!(log.out.is_none());
    }
}
