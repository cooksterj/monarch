//! Removes the host-input sequences Monarch asked for but the pane did not.
//!
//! The host always has bracketed paste and focus reporting on (see
//! [`crate::HostModeMirror::enter`]), so a pane that never enabled them would
//! otherwise receive `ESC [200~`, `ESC [I` and friends it cannot parse. With
//! the pane's mode on, the bytes pass through untouched (G4, G6).
//!
//! A marker split across reads is held back only once it is at least three
//! bytes long (`ESC [2…`); a lone `ESC` or `ESC [` is forwarded at once, so
//! the Escape key and Alt+`[` are never delayed.

use memchr::memchr;
use monarch_proto::ModeFlags;

use super::HostFocus;

const ESC: u8 = 0x1b;
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
const FOCUS_IN: &[u8] = b"\x1b[I";
const FOCUS_OUT: &[u8] = b"\x1b[O";
const MARKERS: [&[u8]; 4] = [PASTE_START, PASTE_END, FOCUS_IN, FOCUS_OUT];
const MIN_HELD: usize = 3;

/// Streaming filter between the prefix parser and the pane.
#[derive(Debug, Clone, Default)]
pub struct HostInputFilter {
    held: Vec<u8>,
    in_paste: bool,
}

impl HostInputFilter {
    /// An empty filter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters one chunk of pane-bound input for a pane with `modes`,
    /// appending the result to `out`. Returns the last focus change seen.
    pub fn filter(
        &mut self,
        bytes: &[u8],
        modes: ModeFlags,
        out: &mut Vec<u8>,
    ) -> Option<HostFocus> {
        let mut data = std::mem::take(&mut self.held);
        data.extend_from_slice(bytes);
        let mut focus = None;
        let mut pos = 0;
        while let Some(off) = memchr(ESC, &data[pos..]) {
            out.extend_from_slice(&data[pos..pos + off]);
            pos += off;
            let rest = &data[pos..];
            if let Some(marker) = MARKERS.into_iter().find(|m| rest.starts_with(m)) {
                focus = self.marker(marker, modes, out).or(focus);
                pos += marker.len();
            } else if is_partial_marker(rest) {
                self.held = rest.to_vec();
                return focus;
            } else {
                out.push(ESC);
                pos += 1;
            }
        }
        out.extend_from_slice(&data[pos..]);
        focus
    }

    /// Handles a complete marker: forwards it if the pane wants it.
    fn marker(&mut self, marker: &[u8], modes: ModeFlags, out: &mut Vec<u8>) -> Option<HostFocus> {
        let (wanted, focus) = match marker {
            PASTE_START => {
                self.in_paste = true;
                (modes.contains(ModeFlags::BRACKETED_PASTE), None)
            }
            PASTE_END => {
                self.in_paste = false;
                (modes.contains(ModeFlags::BRACKETED_PASTE), None)
            }
            // Pasted text is data: a focus sequence inside it is literal.
            _ if self.in_paste => (true, None),
            FOCUS_IN => (
                modes.contains(ModeFlags::FOCUS_IN_OUT),
                Some(HostFocus::Gained),
            ),
            _ => (
                modes.contains(ModeFlags::FOCUS_IN_OUT),
                Some(HostFocus::Lost),
            ),
        };
        if wanted {
            out.extend_from_slice(marker);
        }
        focus
    }
}

fn is_partial_marker(rest: &[u8]) -> bool {
    rest.len() >= MIN_HELD
        && MARKERS
            .iter()
            .any(|m| m.len() > rest.len() && m.starts_with(rest))
}

#[cfg(test)]
mod tests;
