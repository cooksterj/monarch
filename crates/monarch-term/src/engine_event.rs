//! Things a pane's terminal reports while parsing output.

use crate::Osc;

/// An event produced while feeding output to a [`crate::Terminal`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EngineEvent {
    /// Bytes to write back to the PTY (query replies: DA, DSR, OSC 10/11,
    /// kitty keyboard reports, text-area size). Order matters.
    Reply(Vec<u8>),
    /// The program set (`Some`) or reset (`None`) the window title.
    Title(Option<String>),
    /// The program rang the bell.
    Bell,
    /// The program asked to copy text to the clipboard (OSC 52).
    ClipboardStore(String),
    /// An OSC sequence captured by the tap (notifications, cwd, prompt marks).
    Osc(Osc),
}
