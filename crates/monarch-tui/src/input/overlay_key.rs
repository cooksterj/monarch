//! Keys for a pop-up or copy mode, decoded from raw host input.
//!
//! Handles plain bytes, CSI and SS3 sequences (arrows, Home/End, PgUp/PgDn,
//! any modifiers) and kitty keyboard reports, since a focused pane may have
//! turned kitty mode on at the host. Mouse reports, paste markers, Alt
//! chords and unknown sequences are dropped; a paste's text arrives as
//! typed characters. What each key means is up to the receiver.

use super::KittyKey;

const ESC: u8 = 0x1b;
const SHIFT: u8 = 1;
const ALT: u8 = 2;
const CTRL: u8 = 4;
/// Caps Lock and Num Lock, which do not change what a key means.
const LOCKS: u8 = 64 | 128;

/// One key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKey {
    /// A printable character.
    Char(char),
    /// Ctrl and a letter (lower case).
    Ctrl(char),
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Home.
    Home,
    /// End.
    End,
    /// Tab.
    Tab,
    /// Shift-Tab.
    BackTab,
    /// Enter.
    Enter,
    /// Backspace.
    Backspace,
    /// Esc (alone at the end of a read).
    Esc,
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

/// A lone Esc, a CSI or SS3 key, or something to drop.
fn escape(rest: &[u8]) -> (Option<OverlayKey>, usize) {
    match rest.get(1) {
        None | Some(&ESC) => (Some(OverlayKey::Esc), 1),
        Some(b'[') => {
            let end = rest[2..].iter().position(|b| (0x40..=0x7e).contains(b));
            end.map_or((None, rest.len()), |end| {
                (csi(&rest[2..2 + end], rest[2 + end]), end + 3)
            })
        }
        Some(b'O') if rest.len() > 2 => (csi(b"", rest[2]), 3),
        Some(_) => (None, 2),
    }
}

/// The key a CSI (or SS3) sequence with these parameters names.
fn csi(params: &[u8], final_byte: u8) -> Option<OverlayKey> {
    Some(match final_byte {
        b'A' => OverlayKey::Up,
        b'B' => OverlayKey::Down,
        b'C' => OverlayKey::Right,
        b'D' => OverlayKey::Left,
        b'H' => OverlayKey::Home,
        b'F' => OverlayKey::End,
        b'Z' => OverlayKey::BackTab,
        b'~' => match params.split(|&b| b == b';').next()? {
            b"1" | b"7" => OverlayKey::Home,
            b"4" | b"8" => OverlayKey::End,
            b"5" => OverlayKey::PageUp,
            b"6" => OverlayKey::PageDown,
            _ => return None,
        },
        _ => return None,
    })
}

fn plain(rest: &[u8]) -> (Option<OverlayKey>, usize) {
    let key = match rest[0] {
        b'\r' | b'\n' => OverlayKey::Enter,
        0x7f | 0x08 => OverlayKey::Backspace,
        b'\t' => OverlayKey::Tab,
        b @ 0x01..=0x1a => OverlayKey::Ctrl(char::from(b'a' + b - 1)),
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
    let ch = char::from_u32(key.code)?;
    if mods & CTRL != 0 {
        return ch
            .is_ascii_alphabetic()
            .then(|| OverlayKey::Ctrl(ch.to_ascii_lowercase()));
    }
    match key.code {
        27 => Some(OverlayKey::Esc),
        13 => Some(OverlayKey::Enter),
        127 | 8 => Some(OverlayKey::Backspace),
        9 if mods & SHIFT != 0 => Some(OverlayKey::BackTab),
        9 => Some(OverlayKey::Tab),
        _ if mods & ALT != 0 => None,
        code => {
            let ch = char::from_u32(key.shifted.unwrap_or(code))?;
            let ch = if mods & SHIFT != 0 && key.shifted.is_none() {
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
    use OverlayKey::{
        BackTab, Backspace, Char, Ctrl, Down, End, Enter, Esc, Home, Left, PageDown, PageUp, Right,
        Tab, Up,
    };

    #[test]
    fn plain_keys() {
        assert_eq!(
            OverlayKey::decode(b"ab\x7f\r\t"),
            [Char('a'), Char('b'), Backspace, Enter, Tab]
        );
        assert_eq!(
            OverlayKey::decode(b"\x0e\x10\x15\x17\x03\x16"),
            [
                Ctrl('n'),
                Ctrl('p'),
                Ctrl('u'),
                Ctrl('w'),
                Ctrl('c'),
                Ctrl('v')
            ]
        );
        assert_eq!(OverlayKey::decode("é→".as_bytes()), [Char('é'), Char('→')]);
    }

    #[test]
    fn escapes_navigation_keys_and_things_to_drop() {
        assert_eq!(OverlayKey::decode(b"\x1b"), [Esc], "lone Esc");
        assert_eq!(OverlayKey::decode(b"\x1b\x1b"), [Esc, Esc]);
        assert_eq!(
            OverlayKey::decode(b"\x1b[A\x1bOB\x1b[1;5C\x1b[D\x1b[Z"),
            [Up, Down, Right, Left, BackTab]
        );
        assert_eq!(
            OverlayKey::decode(b"\x1b[5~\x1b[6~\x1b[H\x1b[F\x1b[1~\x1b[4~\x1bOH"),
            [PageUp, PageDown, Home, End, Home, End, Home]
        );
        assert_eq!(OverlayKey::decode(b"\x1bx\x1b[15~"), [], "Alt+x, F5");
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
            OverlayKey::decode(b"\x1b[27u\x1b[13u\x1b[127u\x1b[9;2u\x1b[9u"),
            [Esc, Enter, Backspace, BackTab, Tab]
        );
        assert_eq!(
            OverlayKey::decode(b"\x1b[110;5u\x1b[97u\x1b[97;2u\x1b[97:65;2u"),
            [Ctrl('n'), Char('a'), Char('A'), Char('A')]
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
