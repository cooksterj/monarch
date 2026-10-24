//! Repeatable keys: pressed again soon after their command, they repeat it
//! without the prefix (tmux's `bind -r`).

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use monarch_core::Key;

use super::command_escape::{CommandEscape, EscapeStep};
use crate::input::KittyKey;

const ESC: u8 = 0x1b;

/// Which keys repeat, for how long, and until when the window is open.
#[derive(Debug, Clone, Default)]
pub(super) struct RepeatWindow {
    keys: BTreeSet<Key>,
    window: Duration,
    until: Option<Instant>,
}

/// A key found at the start of some input, and how many bytes it took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FoundKey {
    pub key: Key,
    pub len: usize,
    /// The kitty key code, when it came as a kitty report (its release is
    /// swallowed too).
    pub kitty: Option<u32>,
}

impl RepeatWindow {
    /// `keys` repeat for `window` after each use; an empty set or a zero
    /// window turns repeating off.
    pub(super) fn new(keys: impl IntoIterator<Item = Key>, window: Duration) -> Self {
        Self {
            keys: keys.into_iter().collect(),
            window,
            until: None,
        }
    }

    /// Opens (or extends) the window if `key` repeats.
    pub(super) fn after(&mut self, key: Key, now: Instant) {
        self.until =
            (self.keys.contains(&key) && !self.window.is_zero()).then(|| now + self.window);
    }

    /// Whether the window is open at `now`.
    pub(super) fn is_open(&self, now: Instant) -> bool {
        self.until.is_some_and(|until| now < until)
    }

    /// Closes the window (any other input ends it).
    pub(super) fn close(&mut self) {
        self.until = None;
    }

    /// The repeatable key at the start of `rest`, if that is what it holds.
    pub(super) fn key_at(&self, rest: &[u8]) -> Option<FoundKey> {
        let found = match *rest.first()? {
            ESC => kitty_key(rest).or_else(|| sequence_key(rest))?,
            byte => FoundKey {
                key: Key::from_byte(byte).filter(|k| matches!(k, Key::Char(_)))?,
                len: 1,
                kitty: None,
            },
        };
        self.keys.contains(&found.key).then_some(found)
    }
}

fn kitty_key(rest: &[u8]) -> Option<FoundKey> {
    let (key, len) = KittyKey::parse(rest)?;
    if key.release {
        return None;
    }
    Some(FoundKey {
        key: key.ascii().and_then(Key::from_byte)?,
        len,
        kitty: Some(key.code),
    })
}

/// An arrow key sent whole (hosts write each sequence in one go).
fn sequence_key(rest: &[u8]) -> Option<FoundKey> {
    let mut escape = CommandEscape::default();
    escape.start();
    for (i, &byte) in rest.iter().enumerate().skip(1) {
        match escape.push(byte) {
            EscapeStep::More => {}
            EscapeStep::Key(key) => {
                return Some(FoundKey {
                    key,
                    len: i + 1,
                    kitty: None,
                });
            }
            EscapeStep::Mouse(_) | EscapeStep::Unbound => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use monarch_core::Direction;

    use super::*;

    fn window() -> RepeatWindow {
        let keys = [Key::Char('H'), Key::Arrow(Direction::Up)];
        RepeatWindow::new(keys, Duration::from_millis(500))
    }

    #[test]
    fn opens_only_after_a_repeatable_key_and_expires() {
        let now = Instant::now();
        let mut w = window();
        w.after(Key::Char('h'), now);
        assert!(!w.is_open(now));
        w.after(Key::Char('H'), now);
        assert!(w.is_open(now + Duration::from_millis(499)));
        assert!(!w.is_open(now + Duration::from_millis(500)));
        w.close();
        assert!(!w.is_open(now));
    }

    #[test]
    fn a_zero_window_never_opens() {
        let now = Instant::now();
        let mut w = RepeatWindow::new([Key::Char('H')], Duration::ZERO);
        w.after(Key::Char('H'), now);
        assert!(!w.is_open(now));
    }

    #[test]
    fn finds_characters_arrows_and_kitty_reports() {
        let w = window();
        assert_eq!(w.key_at(b"Hx").map(|f| f.len), Some(1));
        assert_eq!(w.key_at(b"\x1b[Ax").map(|f| f.len), Some(3));
        let kitty = w.key_at(b"\x1b[104:72;2u").expect("kitty H");
        assert_eq!((kitty.key, kitty.kitty), (Key::Char('H'), Some(104)));
        assert!(w.key_at(b"h").is_none(), "not repeatable");
        assert!(w.key_at(b"\x1b[B").is_none(), "down does not repeat here");
        assert!(w.key_at(b"\x1b[<0;1;1M").is_none(), "mouse");
        assert!(w.key_at(b"\r").is_none() && w.key_at(b"").is_none());
    }
}
