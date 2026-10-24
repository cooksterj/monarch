//! Reads an escape sequence typed as the command key (arrows, Esc).

use monarch_core::{Direction, Key};

const ESC: u8 = 0x1b;
/// Longer sequences are not keys Monarch binds; they are dropped.
const MAX_LEN: usize = 16;

/// What one more byte of the sequence made of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum EscapeStep {
    /// The sequence continues.
    More,
    /// A key Monarch can bind.
    Key(Key),
    /// An SGR mouse report: it belongs to the pane, and the prefix still
    /// waits for its key.
    Mouse(Vec<u8>),
    /// A key Monarch does not bind (Alt+x, F5…); it is swallowed.
    Unbound,
}

/// The escape sequence after the prefix, collected across reads.
#[derive(Debug, Clone, Default)]
pub(super) struct CommandEscape {
    buf: Vec<u8>,
}

impl CommandEscape {
    /// An `ESC` arrived as the command key.
    pub(super) fn start(&mut self) {
        self.buf.clear();
        self.buf.push(ESC);
    }

    /// Whether a sequence is being collected.
    pub(super) fn is_active(&self) -> bool {
        !self.buf.is_empty()
    }

    /// Adds one byte (CSI `ESC [ … final` or SS3 `ESC O final`).
    pub(super) fn push(&mut self, byte: u8) -> EscapeStep {
        if self.buf.len() == 1 {
            if matches!(byte, b'[' | b'O') {
                self.buf.push(byte);
                return EscapeStep::More;
            }
            self.buf.clear();
            return EscapeStep::Unbound;
        }
        match byte {
            0x20..=0x3f if self.buf.len() < MAX_LEN => {
                self.buf.push(byte);
                EscapeStep::More
            }
            0x40..=0x7e => {
                let seq = std::mem::take(&mut self.buf);
                finish(seq, byte)
            }
            _ => {
                self.buf.clear();
                EscapeStep::Unbound
            }
        }
    }

    /// The end of a read: an `ESC` alone is the Esc key, since hosts send
    /// a whole sequence in one write.
    pub(super) fn end_of_read(&mut self) -> Option<Key> {
        (self.buf == [ESC]).then(|| {
            self.buf.clear();
            Key::Esc
        })
    }
}

fn finish(mut seq: Vec<u8>, last: u8) -> EscapeStep {
    if seq.get(2) == Some(&b'<') {
        seq.push(last);
        return EscapeStep::Mouse(seq);
    }
    let dir = match last {
        b'A' => Direction::Up,
        b'B' => Direction::Down,
        b'C' => Direction::Right,
        b'D' => Direction::Left,
        _ => return EscapeStep::Unbound,
    };
    EscapeStep::Key(Key::Arrow(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(bytes: &[u8]) -> Vec<EscapeStep> {
        let mut esc = CommandEscape::default();
        esc.start();
        bytes.iter().map(|&b| esc.push(b)).collect()
    }

    #[test]
    fn arrows_in_every_encoding() {
        let left = EscapeStep::Key(Key::Arrow(Direction::Left));
        assert_eq!(run(b"[D").last(), Some(&left));
        assert_eq!(run(b"OD").last(), Some(&left), "application cursor");
        assert_eq!(run(b"[1;2D").last(), Some(&left), "with modifiers");
        assert_eq!(
            run(b"[A").last(),
            Some(&EscapeStep::Key(Key::Arrow(Direction::Up)))
        );
    }

    #[test]
    fn mouse_reports_are_handed_back_whole() {
        assert_eq!(
            run(b"[<0;5;9M").last(),
            Some(&EscapeStep::Mouse(b"\x1b[<0;5;9M".to_vec()))
        );
    }

    #[test]
    fn other_sequences_are_unbound() {
        assert_eq!(run(b"x"), [EscapeStep::Unbound], "alt+x");
        assert_eq!(run(b"[15~").last(), Some(&EscapeStep::Unbound), "F5");
        assert_eq!(run(b"[\x01"), [EscapeStep::More, EscapeStep::Unbound]);
        let long = [b"[".as_slice(), &[b'1'; 20]].concat();
        assert!(run(&long).contains(&EscapeStep::Unbound), "too long");
    }

    #[test]
    fn a_lone_esc_is_the_esc_key_only_at_the_end_of_a_read() {
        let mut esc = CommandEscape::default();
        esc.start();
        assert_eq!(esc.end_of_read(), Some(Key::Esc));
        assert!(!esc.is_active());
        esc.start();
        esc.push(b'[');
        assert_eq!(esc.end_of_read(), None, "a split sequence keeps going");
        assert!(esc.is_active());
    }
}
