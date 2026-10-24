//! Splits SGR mouse reports (`CSI < b ; x ; y M|m`) out of host input.
//!
//! Monarch asks the host for SGR mouse reports whenever it captures the
//! mouse, so this is the only encoding it has to recognise. Paste bodies
//! are never scanned. A report split across reads is held back only once
//! it starts with `ESC [ <`, so a lone Escape is never delayed.

use memchr::memchr;

use super::{HostInput, MouseEvent};

const ESC: u8 = 0x1b;
const SGR: &[u8] = b"\x1b[<";
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
/// Longer than any real report; anything longer is passed on as keys.
const MAX_REPORT: usize = 32;

/// Streaming splitter; keep one per input stream.
#[derive(Debug, Clone, Default)]
pub struct MouseExtractor {
    held: Vec<u8>,
    in_paste: bool,
}

impl MouseExtractor {
    /// An empty splitter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Splits `bytes` into key runs and mouse reports, appended to `out`.
    pub fn split(&mut self, bytes: &[u8], out: &mut Vec<HostInput>) {
        let mut data = std::mem::take(&mut self.held);
        data.extend_from_slice(bytes);
        let mut keys = Vec::new();
        let mut pos = 0;
        while let Some(off) = memchr(ESC, &data[pos..]) {
            keys.extend_from_slice(&data[pos..pos + off]);
            pos += off;
            let rest = &data[pos..];
            match self.step(rest) {
                Step::Keys(n) => {
                    keys.extend_from_slice(&rest[..n]);
                    pos += n;
                }
                Step::Skip(n, event) => {
                    if let Some(event) = event {
                        flush(&mut keys, out);
                        out.push(HostInput::Mouse(event));
                    }
                    pos += n;
                }
                Step::Hold => {
                    self.held = rest.to_vec();
                    flush(&mut keys, out);
                    return;
                }
            }
        }
        keys.extend_from_slice(&data[pos..]);
        flush(&mut keys, out);
    }

    /// What to do with `rest`, which starts with `ESC`.
    fn step(&mut self, rest: &[u8]) -> Step {
        if is_partial_marker(rest) {
            return Step::Hold;
        }
        if self.in_paste {
            self.in_paste = !rest.starts_with(PASTE_END);
            return Step::Keys(if self.in_paste { 1 } else { PASTE_END.len() });
        }
        if rest.starts_with(PASTE_START) {
            self.in_paste = true;
            return Step::Keys(PASTE_START.len());
        }
        if !rest.starts_with(SGR) {
            return Step::Keys(1);
        }
        match report(rest) {
            Report::Complete(len, event) => Step::Skip(len, event),
            Report::Partial => Step::Hold,
            Report::Invalid => Step::Keys(1),
        }
    }
}

/// One decision of [`MouseExtractor::split`].
enum Step {
    /// Pass this many bytes on as keys.
    Keys(usize),
    /// Consume this many bytes, emitting the report if it decoded.
    Skip(usize, Option<MouseEvent>),
    /// Keep the rest for the next read.
    Hold,
}

enum Report {
    /// Length consumed and the event, if it decoded.
    Complete(usize, Option<MouseEvent>),
    Partial,
    Invalid,
}

/// Classifies `rest`, which starts with `ESC [ <`.
fn report(rest: &[u8]) -> Report {
    let body = &rest[SGR.len()..];
    match body
        .iter()
        .position(|b| !(b.is_ascii_digit() || *b == b';'))
    {
        Some(i) if matches!(body[i], b'M' | b'm') => {
            let event = MouseEvent::from_sgr(&body[..i], body[i] == b'm');
            Report::Complete(SGR.len() + i + 1, event)
        }
        Some(_) => Report::Invalid,
        None if rest.len() < MAX_REPORT => Report::Partial,
        None => Report::Invalid,
    }
}

/// A paste marker cut short at the end of a read (`ESC [2…`).
fn is_partial_marker(rest: &[u8]) -> bool {
    rest.len() >= 3
        && [PASTE_START, PASTE_END]
            .iter()
            .any(|m| rest.len() < m.len() && m.starts_with(rest))
}

fn flush(keys: &mut Vec<u8>, out: &mut Vec<HostInput>) {
    if !keys.is_empty() {
        out.push(HostInput::Keys(std::mem::take(keys)));
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::input::{MouseButton, MouseKind};

    fn split_all(chunks: &[&[u8]]) -> Vec<HostInput> {
        let mut x = MouseExtractor::new();
        let mut out = Vec::new();
        for chunk in chunks {
            x.split(chunk, &mut out);
        }
        out
    }

    fn keys(out: &[HostInput]) -> Vec<u8> {
        out.iter()
            .flat_map(|p| match p {
                HostInput::Keys(k) => k.clone(),
                HostInput::Mouse(_) => Vec::new(),
            })
            .collect()
    }

    #[test]
    fn reports_are_cut_out_in_order() {
        let out = split_all(&[b"ab\x1b[<0;3;4Mcd"]);
        assert_eq!(out[0], HostInput::Keys(b"ab".to_vec()));
        assert!(matches!(
            out[1],
            HostInput::Mouse(MouseEvent {
                kind: MouseKind::Press(MouseButton::Left),
                col: 2,
                row: 3,
                ..
            })
        ));
        assert_eq!(out[2], HostInput::Keys(b"cd".to_vec()));
    }

    #[test]
    fn a_split_report_is_held_until_complete() {
        let out = split_all(&[b"x\x1b[<64;1", b";1My"]);
        assert_eq!(keys(&out), b"xy");
        assert_eq!(
            out.iter()
                .filter(|p| matches!(p, HostInput::Mouse(_)))
                .count(),
            1
        );
    }

    #[test]
    fn escape_keys_are_never_held() {
        let out = split_all(&[b"\x1b"]);
        assert_eq!(out, [HostInput::Keys(b"\x1b".to_vec())]);
        assert_eq!(keys(&split_all(&[b"\x1b[A\x1b[<x"])), b"\x1b[A\x1b[<x");
    }

    #[test]
    fn paste_bodies_are_not_scanned() {
        let paste = b"\x1b[200~\x1b[<0;1;1M\x1b[201~";
        assert_eq!(split_all(&[paste]), [HostInput::Keys(paste.to_vec())]);
    }

    #[test]
    fn a_split_paste_end_still_ends_the_paste() {
        let out = split_all(&[b"\x1b[200~p\x1b[20", b"1~\x1b[<0;1;1M"]);
        assert_eq!(keys(&out), b"\x1b[200~p\x1b[201~");
        assert!(matches!(out.last(), Some(HostInput::Mouse(_))));
    }

    #[test]
    fn unsupported_reports_are_dropped_not_typed() {
        assert_eq!(keys(&split_all(&[b"a\x1b[<128;1;1Mb"])), b"ab");
    }

    proptest! {
        #[test]
        fn chunking_never_changes_the_result(cut in 0usize..40) {
            let input: &[u8] = b"hi\x1b[<0;10;5M\x1b[A\x1b[<65;2;2mok\x1b";
            let cut = cut.min(input.len());
            // A read ending in `ESC` or `ESC [` is sent on at once (Escape
            // is never delayed), so those cuts are out of scope.
            prop_assume!(!input[..cut].ends_with(b"\x1b") && !input[..cut].ends_with(b"\x1b["));
            let whole = split_all(&[input]);
            let parts = split_all(&[&input[..cut], &input[cut..]]);
            prop_assert_eq!(keys(&whole), keys(&parts));
            let mice = |o: &[HostInput]| o.iter().filter(|p| matches!(p, HostInput::Mouse(_))).cloned().collect::<Vec<_>>();
            prop_assert_eq!(mice(&whole), mice(&parts));
        }
    }
}
