//! Finds Monarch's prefix chord in raw host input without delaying or
//! altering anything else (Terminal Fidelity Contract G1).
//!
//! The prefix is a single control byte, so recognising it needs no
//! look-ahead and no escape timeout. Bracketed-paste bodies are never
//! scanned: a prefix byte inside a paste belongs to the pane. Pressing the
//! prefix twice sends one literal prefix byte to the pane, as in tmux.
//!
//! When the pane has kitty keyboard flags on, the host reports the prefix
//! as `CSI 98;5u` (Ctrl+b) instead of `0x02`, and may report the command
//! key and key releases as `CSI … u` too; both forms are recognised, and the
//! releases of keys Monarch consumed are swallowed so the pane never sees a
//! release without its press.
//!
//! An escape sequence as the command key is read as an arrow (CSI or SS3,
//! any modifiers) or, alone at the end of a read, as Esc; other sequences
//! are swallowed rather than typed into the pane.
//!
//! After a repeatable command (resize, swap), its key pressed again within
//! the repeat window runs it again without the prefix; any other input
//! closes the window and reaches the pane as usual.

mod command_escape;
mod repeat_window;

use std::time::{Duration, Instant};

use memchr::{memchr, memchr2};
use monarch_core::Key;

use self::command_escape::{CommandEscape, EscapeStep};
use self::repeat_window::RepeatWindow;
use super::{InputEvent, KittyKey};

const ESC: u8 = 0x1b;
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
/// Releases remembered for swallowing; hosts without release events never
/// drain the list, so it is kept short.
const MAX_SWALLOW: usize = 4;

/// Streaming prefix detector; keeps state across `read()` boundaries.
#[derive(Debug, Clone)]
pub struct PrefixParser {
    prefix: u8,
    pending_prefix: bool,
    in_paste: bool,
    marker_progress: usize,
    swallow: Vec<u32>,
    escape: CommandEscape,
    repeat: RepeatWindow,
    /// When the chunk being parsed arrived.
    now: Instant,
}

impl PrefixParser {
    /// A parser for the given prefix byte (`0x02` for `Ctrl-b`).
    #[must_use]
    pub fn new(prefix: u8) -> Self {
        Self {
            prefix,
            pending_prefix: false,
            in_paste: false,
            marker_progress: 0,
            swallow: Vec::new(),
            escape: CommandEscape::default(),
            repeat: RepeatWindow::default(),
            now: Instant::now(),
        }
    }

    /// Lets `keys` repeat without the prefix for `window` after each use.
    #[must_use]
    pub fn with_repeat(mut self, keys: impl IntoIterator<Item = Key>, window: Duration) -> Self {
        self.repeat = RepeatWindow::new(keys, window);
        self
    }

    /// Whether the prefix was pressed and Monarch is waiting for the key.
    #[must_use]
    pub fn awaiting_command(&self) -> bool {
        self.pending_prefix || self.escape.is_active()
    }

    /// Parses one chunk of host input, appending events to `out`.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<InputEvent>) {
        self.feed_at(bytes, Instant::now(), out);
    }

    /// [`feed`](Self::feed) for input that arrived at `now`.
    pub fn feed_at(&mut self, bytes: &[u8], now: Instant, out: &mut Vec<InputEvent>) {
        self.now = now;
        let mut forward = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if self.escape.is_active() {
                self.on_escape_byte(bytes[i], &mut forward, out);
                i += 1;
                continue;
            }
            if let Some(len) = self.try_repeat(&bytes[i..], &mut forward, out) {
                i += len;
                continue;
            }
            match self.skip_plain(&bytes[i..], &mut forward) {
                Some(skipped) => i += skipped,
                None => break,
            }
            if let Some((key, len)) = self.kitty_key_at(&bytes[i..]) {
                self.on_kitty_key(key, &bytes[i..i + len], &mut forward, out);
                i += len;
                continue;
            }
            if self.pending_prefix {
                self.pending_prefix = false;
                self.after_prefix(bytes[i], &mut forward, out);
            } else {
                self.step(bytes[i], &mut forward, out);
            }
            i += 1;
        }
        if let Some(key) = self.escape.end_of_read() {
            self.command(key, &mut forward, out);
        }
        flush(&mut forward, out);
    }

    /// Emits a command key, opening the repeat window if it repeats.
    fn command(&mut self, key: Key, forward: &mut Vec<u8>, out: &mut Vec<InputEvent>) {
        flush(forward, out);
        out.push(InputEvent::Command(key));
        self.repeat.after(key, self.now);
    }

    /// While the repeat window is open, a repeatable key at the start of
    /// `rest` is a command; anything else closes the window. Returns the
    /// bytes consumed.
    fn try_repeat(
        &mut self,
        rest: &[u8],
        forward: &mut Vec<u8>,
        out: &mut Vec<InputEvent>,
    ) -> Option<usize> {
        let idle = !self.pending_prefix && !self.in_paste && self.marker_progress == 0;
        if !idle || !self.repeat.is_open(self.now) {
            return None;
        }
        let Some(found) = self.repeat.key_at(rest) else {
            self.repeat.close();
            return None;
        };
        if let Some(code) = found.kitty {
            self.remember_release(code);
        }
        self.command(found.key, forward, out);
        Some(found.len)
    }

    fn on_escape_byte(&mut self, byte: u8, forward: &mut Vec<u8>, out: &mut Vec<InputEvent>) {
        match self.escape.push(byte) {
            EscapeStep::More | EscapeStep::Unbound => {}
            EscapeStep::Key(key) => self.command(key, forward, out),
            EscapeStep::Mouse(report) => {
                forward.extend_from_slice(&report);
                self.pending_prefix = true;
            }
        }
    }

    /// Forwards plain bytes up to the next one that could matter; `None`
    /// when the rest of the chunk was plain.
    fn skip_plain(&self, rest: &[u8], forward: &mut Vec<u8>) -> Option<usize> {
        if self.marker_progress != 0 || self.pending_prefix {
            return Some(0);
        }
        let Some(off) = self.next_interesting(rest) else {
            forward.extend_from_slice(rest);
            return None;
        };
        forward.extend_from_slice(&rest[..off]);
        Some(off)
    }

    /// Offset of the next byte that could start a prefix or paste marker.
    fn next_interesting(&self, rest: &[u8]) -> Option<usize> {
        if self.in_paste {
            memchr(ESC, rest)
        } else {
            memchr2(self.prefix, ESC, rest)
        }
    }

    /// A kitty key report starting at `rest`, outside any paste.
    fn kitty_key_at(&self, rest: &[u8]) -> Option<(KittyKey, usize)> {
        if self.in_paste || rest.first() != Some(&ESC) {
            return None;
        }
        KittyKey::parse(rest)
    }

    fn on_kitty_key(
        &mut self,
        key: KittyKey,
        raw: &[u8],
        forward: &mut Vec<u8>,
        out: &mut Vec<InputEvent>,
    ) {
        self.marker_progress = 0;
        if key.release {
            match self.swallow.iter().position(|&code| code == key.code) {
                Some(found) => drop(self.swallow.remove(found)),
                None => forward.extend_from_slice(raw),
            }
        } else if self.pending_prefix {
            self.pending_prefix = false;
            if key.is_ctrl_byte(self.prefix) {
                forward.extend_from_slice(raw);
            } else if let Some(command) = key.ascii().and_then(Key::from_byte) {
                self.command(command, forward, out);
                self.remember_release(key.code);
            }
        } else if key.is_ctrl_byte(self.prefix) {
            flush(forward, out);
            self.pending_prefix = true;
            self.remember_release(key.code);
        } else {
            forward.extend_from_slice(raw);
        }
    }

    fn remember_release(&mut self, code: u32) {
        if self.swallow.len() == MAX_SWALLOW {
            self.swallow.remove(0);
        }
        self.swallow.push(code);
    }

    fn step(&mut self, byte: u8, forward: &mut Vec<u8>, out: &mut Vec<InputEvent>) {
        if !self.in_paste && byte == self.prefix {
            self.marker_progress = 0;
            flush(forward, out);
            self.pending_prefix = true;
            return;
        }
        forward.push(byte);
        self.advance_marker(byte);
    }

    fn after_prefix(&mut self, byte: u8, forward: &mut Vec<u8>, out: &mut Vec<InputEvent>) {
        if byte == self.prefix {
            forward.push(byte);
        } else if byte == ESC {
            self.escape.start();
        } else if let Some(key) = Key::from_byte(byte) {
            self.command(key, forward, out);
        }
    }

    fn advance_marker(&mut self, byte: u8) {
        let target = if self.in_paste {
            PASTE_END
        } else {
            PASTE_START
        };
        if byte == target[self.marker_progress] {
            self.marker_progress += 1;
            if self.marker_progress == target.len() {
                self.in_paste = !self.in_paste;
                self.marker_progress = 0;
            }
        } else {
            self.marker_progress = usize::from(byte == ESC);
        }
    }
}

fn flush(forward: &mut Vec<u8>, out: &mut Vec<InputEvent>) {
    if !forward.is_empty() {
        out.push(InputEvent::Forward(std::mem::take(forward)));
    }
}

#[cfg(test)]
mod tests;
