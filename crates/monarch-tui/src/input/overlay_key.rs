//! Keys for an open pop-up (palette, picker), decoded from raw host input.
//!
//! Handles plain bytes, arrows (CSI or SS3, any modifiers) and kitty
//! keyboard reports, since a focused pane may have turned kitty mode on at
//! the host. Mouse reports, paste markers, Alt chords and unknown sequences
//! are dropped; a paste's text arrives as typed characters.

use super::KittyKey;

const ESC: u8 = 0x1b;
const CTRL: u8 = 4;
const ALT: u8 = 2;
/// Caps Lock and Num Lock, which do not change what a key means.
const LOCKS: u8 = 64 | 128;

/// One pop-up key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKey {
    /// A character for the query.
    Char(char),
    /// Delete the character before the cursor.
    Backspace,
    /// Delete the word before the cursor (Ctrl-w).
    DeleteWord,
    /// Clear the query (Ctrl-u).
    Clear,
    /// Highlight the row above (Up, Ctrl-p, Shift-Tab).
    Up,
    /// Highlight the row below (Down, Ctrl-n, Tab).
    Down,
    /// Run the highlighted row.
    Enter,
    /// Close without running anything (Esc, Ctrl-c, Ctrl-g).
    Cancel,
}

impl OverlayKey {
    /// Every key in one read of host input.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Vec<Self> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let rest = &bytes[i..];
            let (key, len) = if rest[0] == ESC {
                KittyKey::parse(rest).map_or_else(
                    || escape(rest),
                    |(k, len)| ((!k.release).then(|| kitty(k)).flatten(), len),
                )
            } else {
                plain(rest)
            };
            out.extend(key);
            i += len.max(1);
        }
        out
    }
}

/// A lone Esc at the end of a read, an arrow, or something to drop.
fn escape(rest: &[u8]) -> (Option<OverlayKey>, usize) {
    match rest.get(1) {
        None | Some(&ESC) => (Some(OverlayKey::Cancel), 1),
        Some(b'[') => {
            let end = rest[2..].iter().position(|b| (0x40..=0x7e).contains(b));
            end.map_or((None, rest.len()), |end| (arrow(rest[2 + end]), end + 3))
        }
        Some(b'O') if rest.len() > 2 => (arrow(rest[2]), 3),
        Some(_) => (None, 2),
    }
}

/// Up, Down, or Shift-Tab (`CSI Z`, which moves up).
fn arrow(final_byte: u8) -> Option<OverlayKey> {
    match final_byte {
        b'A' | b'Z' => Some(OverlayKey::Up),
        b'B' => Some(OverlayKey::Down),
        _ => None,
    }
}

fn plain(rest: &[u8]) -> (Option<OverlayKey>, usize) {
    let key = match rest[0] {
        b'\r' | b'\n' => OverlayKey::Enter,
        0x7f | 0x08 => OverlayKey::Backspace,
        b'\t' | 0x0e => OverlayKey::Down,
        0x10 => OverlayKey::Up,
        0x15 => OverlayKey::Clear,
        0x17 => OverlayKey::DeleteWord,
        0x03 | 0x07 => OverlayKey::Cancel,
        0..0x20 => return (None, 1),
        lead => {
            let len = utf8_len(lead).min(rest.len());
            let ch = std::str::from_utf8(&rest[..len])
                .ok()
                .and_then(|s| s.chars().next());
            return (ch.map(OverlayKey::Char), len);
        }
    };
    (Some(key), 1)
}

fn utf8_len(lead: u8) -> usize {
    match lead.leading_ones() {
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 1,
    }
}

/// A kitty key press.
fn kitty(key: KittyKey) -> Option<OverlayKey> {
    let mods = key.mods & !LOCKS;
    if mods & CTRL != 0 {
        return match char::from_u32(key.code)? {
            'n' => Some(OverlayKey::Down),
            'p' => Some(OverlayKey::Up),
            'u' => Some(OverlayKey::Clear),
            'w' => Some(OverlayKey::DeleteWord),
            'c' | 'g' => Some(OverlayKey::Cancel),
            _ => None,
        };
    }
    match key.code {
        27 => Some(OverlayKey::Cancel),
        13 => Some(OverlayKey::Enter),
        127 | 8 => Some(OverlayKey::Backspace),
        9 if mods & 1 != 0 => Some(OverlayKey::Up),
        9 => Some(OverlayKey::Down),
        _ if mods & ALT != 0 => None,
        code => {
            let ch = char::from_u32(key.shifted.unwrap_or(code))?;
            let ch = if mods & 1 != 0 && key.shifted.is_none() {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            (!ch.is_control()).then_some(OverlayKey::Char(ch))
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use OverlayKey::{Backspace, Cancel, Char, Clear, DeleteWord, Down, Enter, Up};

    #[test]
    fn plain_keys() {
        assert_eq!(
            OverlayKey::decode(b"ab\x7f\r"),
            [Char('a'), Char('b'), Backspace, Enter]
        );
        assert_eq!(
            OverlayKey::decode(b"\x0e\x10\t\x15\x17\x03"),
            [Down, Up, Down, Clear, DeleteWord, Cancel]
        );
        assert_eq!(OverlayKey::decode("é→".as_bytes()), [Char('é'), Char('→')]);
    }

    #[test]
    fn escapes_arrows_and_things_to_drop() {
        assert_eq!(OverlayKey::decode(b"\x1b"), [Cancel], "lone Esc");
        assert_eq!(OverlayKey::decode(b"\x1b\x1b"), [Cancel, Cancel]);
        assert_eq!(
            OverlayKey::decode(b"\x1b[A\x1bOB\x1b[1;5A\x1b[Z"),
            [Up, Down, Up, Up]
        );
        assert_eq!(OverlayKey::decode(b"\x1bx"), [], "Alt+x");
        assert_eq!(OverlayKey::decode(b"\x1b[<0;3;4M\x1b[<0;3;4m"), [], "mouse");
        assert_eq!(
            OverlayKey::decode(b"\x1b[200~hi\x1b[201~"),
            [Char('h'), Char('i')],
            "a paste is typed"
        );
        assert_eq!(OverlayKey::decode(b"\x1b[1;5"), [], "cut off");
    }

    #[test]
    fn kitty_reports() {
        assert_eq!(
            OverlayKey::decode(b"\x1b[27u\x1b[13u\x1b[127u\x1b[9;2u"),
            [Cancel, Enter, Backspace, Up]
        );
        assert_eq!(
            OverlayKey::decode(b"\x1b[110;5u\x1b[97u\x1b[97;2u\x1b[97:65;2u"),
            [Down, Char('a'), Char('A'), Char('A')]
        );
        assert_eq!(OverlayKey::decode(b"\x1b[97;1:3u"), [], "release");
        assert_eq!(OverlayKey::decode(b"\x1b[97;3u"), [], "Alt+a");
        assert_eq!(OverlayKey::decode(b"\x1b[97;65u"), [Char('a')], "caps lock");
    }

    proptest! {
        #[test]
        fn never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            let _ = OverlayKey::decode(&bytes);
        }
    }
}
