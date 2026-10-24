//! Puts text on the host terminal's clipboard with OSC 52 (G5).

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Encodes clipboard writes for the host terminal. Works over SSH, but
/// some terminals (iTerm2 by default) ignore it, so local clients also use
/// the system clipboard (see `crate::SystemClipboard`).
#[derive(Debug, Clone, Copy, Default)]
pub struct HostClipboard;

impl HostClipboard {
    /// `OSC 52 ; c ; <base64> ST`: set the clipboard to `text`.
    #[must_use]
    pub fn osc52(text: &str) -> Vec<u8> {
        let mut out = b"\x1b]52;c;".to_vec();
        out.extend_from_slice(STANDARD.encode(text).as_bytes());
        out.extend_from_slice(b"\x1b\\");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_utf8_text_as_base64() {
        assert_eq!(HostClipboard::osc52("hi"), b"\x1b]52;c;aGk=\x1b\\");
        assert_eq!(HostClipboard::osc52("é"), b"\x1b]52;c;w6k=\x1b\\");
        assert_eq!(HostClipboard::osc52(""), b"\x1b]52;c;\x1b\\");
    }
}
