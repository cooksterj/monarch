//! Finds Monarch's prefix chord in raw host input without delaying or
//! altering anything else (Terminal Fidelity Contract G1).
//!
//! The prefix is a single control byte, so recognising it needs no
//! look-ahead and no escape timeout. Bracketed-paste bodies are never
//! scanned: a prefix byte inside a paste belongs to the pane. Pressing the
//! prefix twice sends one literal prefix byte to the pane, as in tmux.

use memchr::{memchr, memchr2};

use super::InputEvent;

const ESC: u8 = 0x1b;
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// Streaming prefix detector; keeps state across `read()` boundaries.
#[derive(Debug, Clone)]
pub struct PrefixParser {
    prefix: u8,
    pending_prefix: bool,
    in_paste: bool,
    marker_progress: usize,
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
        }
    }

    /// Whether the prefix was pressed and Monarch is waiting for the key.
    #[must_use]
    pub fn awaiting_command(&self) -> bool {
        self.pending_prefix
    }

    /// Parses one chunk of host input, appending events to `out`.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<InputEvent>) {
        let mut forward = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if self.pending_prefix {
                self.pending_prefix = false;
                self.after_prefix(bytes[i], &mut forward, out);
                i += 1;
                continue;
            }
            if self.marker_progress == 0 {
                let rest = &bytes[i..];
                match self.next_interesting(rest) {
                    Some(off) => {
                        forward.extend_from_slice(&rest[..off]);
                        i += off;
                    }
                    None => {
                        forward.extend_from_slice(rest);
                        break;
                    }
                }
            }
            self.step(bytes[i], &mut forward, out);
            i += 1;
        }
        flush(&mut forward, out);
    }

    /// Offset of the next byte that could start a prefix or paste marker.
    fn next_interesting(&self, rest: &[u8]) -> Option<usize> {
        if self.in_paste {
            memchr(ESC, rest)
        } else {
            memchr2(self.prefix, ESC, rest)
        }
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
        } else {
            flush(forward, out);
            out.push(InputEvent::Command(byte));
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
