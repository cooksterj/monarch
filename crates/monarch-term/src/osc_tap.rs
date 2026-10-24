//! A byte-level scanner that captures OSC sequences before the VT parser.
//!
//! `vte` keeps its `Performer` private and only debug-logs OSC numbers it
//! does not handle, so notifications (OSC 9/99/777), cwd (OSC 7) and prompt
//! marks (OSC 133) would be lost. The tap runs over the same bytes first and
//! follows vte's OSC-string rules exactly, so it sees precisely the OSCs the
//! engine sees. Plain text costs one `memchr` per chunk.

use memchr::memchr;

use crate::Osc;

/// Longest OSC body kept; longer ones are dropped (vte has no limit, but no
/// sequence we capture is anywhere near this size).
const MAX_BODY: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    Osc,
}

/// Streaming OSC scanner; keeps state across chunk boundaries.
#[derive(Debug)]
pub struct OscTap {
    state: State,
    body: Vec<u8>,
    overflowed: bool,
}

impl Default for OscTap {
    fn default() -> Self {
        Self {
            state: State::Ground,
            body: Vec::new(),
            overflowed: false,
        }
    }
}

impl OscTap {
    /// A tap in the ground state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans `bytes`, appending each completed captured OSC to `out`.
    pub fn scan(&mut self, bytes: &[u8], out: &mut Vec<Osc>) {
        let mut i = 0;
        while i < bytes.len() {
            if self.state == State::Ground {
                match memchr(0x1b, &bytes[i..]) {
                    Some(off) => {
                        self.state = State::Escape;
                        i += off + 1;
                    }
                    None => return,
                }
                continue;
            }
            self.step(bytes[i], out);
            i += 1;
        }
    }

    fn step(&mut self, byte: u8, out: &mut Vec<Osc>) {
        match self.state {
            State::Ground => {}
            State::Escape => self.step_escape(byte),
            State::Osc => self.step_osc(byte, out),
        }
    }

    fn step_escape(&mut self, byte: u8) {
        self.state = match byte {
            b']' => {
                self.body.clear();
                self.overflowed = false;
                State::Osc
            }
            0x1b => State::Escape,
            _ => State::Ground,
        };
    }

    /// Mirrors `vte::Parser::advance_osc_string`.
    fn step_osc(&mut self, byte: u8, out: &mut Vec<Osc>) {
        match byte {
            0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1c..=0x1f => {}
            0x07 | 0x18 | 0x1a => {
                self.finish(out);
                self.state = State::Ground;
            }
            0x1b => {
                self.finish(out);
                self.state = State::Escape;
            }
            _ if self.body.len() < MAX_BODY => self.body.push(byte),
            _ => self.overflowed = true,
        }
    }

    fn finish(&mut self, out: &mut Vec<Osc>) {
        if !self.overflowed {
            out.extend(Osc::parse(&self.body));
        }
        self.body.clear();
    }
}

#[cfg(test)]
mod tests;
