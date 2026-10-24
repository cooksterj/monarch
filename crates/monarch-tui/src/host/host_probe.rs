//! Asks the host terminal what it supports and collects the answers.
//!
//! [`HostProbe::QUERY`] ends with a primary device attributes request, which
//! every terminal answers, so its reply marks the end of the probe. Bytes
//! that are not probe replies (keys typed meanwhile) are handed back as
//! input, in order.

use memchr::memchr;
use monarch_proto::HostInfo;

use super::HostReply;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
/// Longest reply worth buffering; anything longer is not one of ours.
const MAX_REPLY: usize = 256;

/// Streaming collector for the replies to [`HostProbe::QUERY`].
#[derive(Debug, Clone, Default)]
pub struct HostProbe {
    info: HostInfo,
    pending: Vec<u8>,
    done: bool,
}

/// Where a buffered sequence stands.
enum Scan {
    Complete(usize),
    Partial,
    NotReply,
}

impl HostProbe {
    /// OSC 10/11/12 colours, cell size, DECRQM 2026, kitty keyboard flags,
    /// then DA1 as the terminator.
    pub const QUERY: &'static [u8] = b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\\
\x1b[16t\x1b[?2026$p\x1b[?u\x1b[c";

    /// A probe waiting for replies.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the terminating DA1 reply has arrived.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// What was learned so far.
    #[must_use]
    pub fn info(&self) -> HostInfo {
        self.info
    }

    /// Consumes host input; non-reply bytes are appended to `input`.
    /// Returns `true` once the probe is complete; after that every byte is
    /// input.
    pub fn feed(&mut self, bytes: &[u8], input: &mut Vec<u8>) -> bool {
        if self.done {
            input.extend_from_slice(bytes);
            return true;
        }
        self.pending.extend_from_slice(bytes);
        let consumed = self.drain(input);
        self.pending.drain(..consumed);
        if self.done {
            input.append(&mut self.pending);
        }
        self.done
    }

    /// Gives up waiting (timeout): buffered bytes become input.
    pub fn abandon(&mut self, input: &mut Vec<u8>) {
        input.append(&mut self.pending);
        self.done = true;
    }

    /// Walks `pending`, returning how many bytes were consumed.
    fn drain(&mut self, input: &mut Vec<u8>) -> usize {
        let mut pos = 0;
        while !self.done && pos < self.pending.len() {
            let rest = &self.pending[pos..];
            let Some(esc) = memchr(ESC, rest) else {
                input.extend_from_slice(rest);
                return self.pending.len();
            };
            input.extend_from_slice(&rest[..esc]);
            pos += esc;
            match scan(&self.pending[pos..]) {
                Scan::Partial => return pos,
                Scan::NotReply => {
                    input.push(ESC);
                    pos += 1;
                }
                Scan::Complete(len) => {
                    let seq = pos..pos + len;
                    match HostReply::parse(&self.pending[seq.clone()]) {
                        Some(reply) => self.apply(reply),
                        None => input.extend_from_slice(&self.pending[seq]),
                    }
                    pos += len;
                }
            }
        }
        pos
    }

    fn apply(&mut self, reply: HostReply) {
        let info = &mut self.info;
        match reply {
            HostReply::Color { slot: 10, rgb } => info.foreground = Some(rgb),
            HostReply::Color { slot: 11, rgb } => info.background = Some(rgb),
            HostReply::Color { rgb, .. } => info.cursor = Some(rgb),
            HostReply::CellSize { width, height } => info.cell_px = Some((width, height)),
            HostReply::SyncOutput(on) => info.sync_output = on,
            HostReply::KittyFlags(flags) => info.kitty_flags = Some(flags),
            HostReply::DeviceAttributes => self.done = true,
        }
    }
}

/// Finds the end of the OSC or CSI sequence at the start of `seq`.
fn scan(seq: &[u8]) -> Scan {
    match seq.get(1) {
        None => Scan::Partial,
        Some(b']') => scan_osc(seq),
        Some(b'[') => scan_csi(seq),
        Some(_) => Scan::NotReply,
    }
}

fn scan_osc(seq: &[u8]) -> Scan {
    for (i, pair) in seq.windows(2).enumerate().skip(1) {
        match pair {
            [BEL, _] => return Scan::Complete(i + 1),
            [ESC, b'\\'] => return Scan::Complete(i + 2),
            _ => {}
        }
    }
    match seq.last() {
        Some(&BEL) => Scan::Complete(seq.len()),
        _ if seq.len() > MAX_REPLY => Scan::NotReply,
        _ => Scan::Partial,
    }
}

fn scan_csi(seq: &[u8]) -> Scan {
    for (i, &byte) in seq.iter().enumerate().skip(2) {
        match byte {
            0x20..=0x3f => {}
            0x40..=0x7e => return Scan::Complete(i + 1),
            _ => return Scan::NotReply,
        }
    }
    if seq.len() > MAX_REPLY {
        Scan::NotReply
    } else {
        Scan::Partial
    }
}

#[cfg(test)]
mod tests;
