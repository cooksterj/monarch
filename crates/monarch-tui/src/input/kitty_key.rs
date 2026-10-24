//! Decodes one kitty keyboard protocol key report (`CSI … u`).
//!
//! Format: `ESC [ code[:shifted[:base]] [; mods[:event]] [; text] u`, where
//! `mods` is one plus a bit set (shift 1, alt 2, ctrl 4, super 8, …) and
//! `event` is 1 press, 2 repeat, 3 release.

/// Longest report worth decoding; real ones are well under this.
const MAX_LEN: usize = 48;
const CTRL: u8 = 4;
/// Caps Lock and Num Lock: reported but irrelevant to chord matching.
const LOCKS: u8 = 64 | 128;

/// One decoded key report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KittyKey {
    /// Unicode codepoint of the unshifted key.
    pub code: u32,
    /// Codepoint with Shift applied, when the host reports alternate keys.
    pub shifted: Option<u32>,
    /// Modifier bits (the wire value minus one).
    pub mods: u8,
    /// Whether this is a key release (only sent with flag 2).
    pub release: bool,
}

impl KittyKey {
    /// Decodes the report at the start of `bytes`, returning it and its
    /// length; `None` if `bytes` does not start with a complete report.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<(Self, usize)> {
        let body = bytes.strip_prefix(b"\x1b[")?;
        let end = body
            .iter()
            .take(MAX_LEN)
            .position(|&b| !(b.is_ascii_digit() || b == b':' || b == b';'))?;
        if body[end] != b'u' {
            return None;
        }
        let text = std::str::from_utf8(&body[..end]).ok()?;
        Some((Self::from_fields(text)?, end + 3))
    }

    fn from_fields(text: &str) -> Option<Self> {
        let mut fields = text.split(';');
        let mut keys = fields.next()?.split(':');
        let code = keys.next()?.parse().ok()?;
        let shifted = keys.next().and_then(|s| s.parse().ok());
        let mut mods = fields.next().unwrap_or("1").split(':');
        let wire: u16 = mods
            .next()
            .filter(|m| !m.is_empty())
            .unwrap_or("1")
            .parse()
            .ok()?;
        let event = mods.next().unwrap_or("1");
        Some(Self {
            code,
            shifted,
            mods: u8::try_from(wire.checked_sub(1)?).ok()?,
            release: event == "3",
        })
    }

    /// Whether this is `Ctrl` + the key whose control byte is `byte`
    /// (`0x02` → Ctrl+b), ignoring lock keys.
    #[must_use]
    pub fn is_ctrl_byte(&self, byte: u8) -> bool {
        self.mods & !LOCKS == CTRL && ctrl_codepoint(byte) == Some(self.code)
    }

    /// The ASCII byte this key types, Shift applied, if it is one.
    #[must_use]
    pub fn ascii(&self) -> Option<u8> {
        u8::try_from(self.shifted.unwrap_or(self.code))
            .ok()
            .filter(u8::is_ascii)
    }
}

/// The key that produces a C0 control byte with Ctrl held.
fn ctrl_codepoint(byte: u8) -> Option<u32> {
    match byte {
        0x00 => Some(u32::from(b' ')),
        0x01..=0x1a => Some(u32::from(byte + 0x60)),
        0x1b..=0x1f => Some(u32::from(byte + 0x40)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(bytes: &[u8]) -> KittyKey {
        let (key, len) = KittyKey::parse(bytes).expect("a kitty key");
        assert_eq!(len, bytes.len());
        key
    }

    #[test]
    fn ctrl_b_in_its_encodings() {
        for seq in [
            &b"\x1b[98;5u"[..],
            b"\x1b[98;5:1u",
            b"\x1b[98;69u",
            b"\x1b[98:66;5u",
        ] {
            assert!(key(seq).is_ctrl_byte(0x02), "{seq:?}");
        }
        assert!(key(b"\x1b[32;5u").is_ctrl_byte(0x00));
        assert!(
            !key(b"\x1b[98;7u").is_ctrl_byte(0x02),
            "ctrl+alt is a different chord"
        );
        assert!(!key(b"\x1b[98u").is_ctrl_byte(0x02));
    }

    #[test]
    fn release_and_text_fields() {
        let k = key(b"\x1b[113;1:3u");
        assert!(k.release);
        assert_eq!(k.ascii(), Some(b'q'));
        assert_eq!(key(b"\x1b[53:37;2u").ascii(), Some(b'%'));
        assert_eq!(key(b"\x1b[97;;97u").ascii(), Some(b'a'));
    }

    #[test]
    fn non_reports_are_rejected() {
        for seq in [
            &b"\x1b[A"[..],
            b"\x1b[1;5A",
            b"\x1b[15~",
            b"\x1b[98;5",
            b"\x1bx",
            b"\x1b[u",
            b"\x1b[98;0u",
        ] {
            assert_eq!(KittyKey::parse(seq), None, "{seq:?}");
        }
    }

    #[test]
    fn only_the_leading_report_is_consumed() {
        assert_eq!(KittyKey::parse(b"\x1b[98;5uq").map(|(_, n)| n), Some(7));
    }
}
