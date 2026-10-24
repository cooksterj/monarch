//! Mirrors the focused pane's terminal modes onto the host terminal, so the
//! host encodes keys, mouse and paste exactly as the pane expects
//! (Terminal Fidelity Contract G2–G7).
//!
//! Only changes are written. Kitty keyboard flags are set absolutely with
//! `CSI = bits ; 1 u` on top of one push made on entry, never pushed and
//! popped per pane, so the host's flag stack cannot drift.

use monarch_core::MouseMode;
use monarch_proto::{Cursor, CursorShape, HostInfo, ModeFlags, TerminalModes};

/// DEC private modes copied verbatim from pane to host, with their numbers.
const DEC_MODES: [(ModeFlags, u16); 7] = [
    (ModeFlags::APP_CURSOR, 1),
    (ModeFlags::MOUSE_CLICK, 1000),
    (ModeFlags::MOUSE_DRAG, 1002),
    (ModeFlags::MOUSE_MOTION, 1003),
    (ModeFlags::UTF8_MOUSE, 1005),
    (ModeFlags::SGR_MOUSE, 1006),
    (ModeFlags::ALTERNATE_SCROLL, 1007),
];

/// What the host was last told.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Applied {
    flags: ModeFlags,
    kitty_flags: u8,
    cursor: (CursorShape, bool),
}

/// Writes the escape sequences that keep the host in step with a pane.
#[derive(Debug, Clone, Default)]
pub struct HostModeMirror {
    kitty: bool,
    mouse: Option<MouseMode>,
    applied: Option<Applied>,
    title: Option<String>,
}

impl HostModeMirror {
    /// A mirror for a host with the given capabilities.
    #[must_use]
    pub fn new(host: &HostInfo) -> Self {
        Self {
            kitty: host.kitty_keyboard(),
            ..Self::default()
        }
    }

    /// Captures the mouse by `policy` instead of copying the pane's mouse
    /// modes verbatim. Captured reports are always SGR, which the client
    /// decodes and re-encodes for the pane (see `MouseRouter`).
    #[must_use]
    pub fn with_mouse(mut self, policy: MouseMode) -> Self {
        self.mouse = Some(policy);
        self
    }

    /// Modes Monarch itself needs on the host: bracketed paste and focus
    /// reporting stay on (input is filtered per pane), the title is saved,
    /// and one kitty keyboard level is pushed.
    pub fn enter(&mut self, out: &mut Vec<u8>) {
        out.extend_from_slice(b"\x1b[22;0t\x1b[?2004h\x1b[?1004h");
        if self.kitty {
            out.extend_from_slice(b"\x1b[>0u");
        }
        self.invalidate();
    }

    /// Restores the host: every mirrored mode off, kitty level popped,
    /// default cursor, title restored.
    pub fn leave(&mut self, out: &mut Vec<u8>) {
        for (_, mode) in DEC_MODES {
            dec(out, mode, false);
        }
        out.extend_from_slice(b"\x1b>\x1b[0 q\x1b[?1004l\x1b[?2004l");
        if self.kitty {
            out.extend_from_slice(b"\x1b[<u");
        }
        out.extend_from_slice(b"\x1b[23;0t");
        self.invalidate();
    }

    /// Forgets what the host was told, so the next [`Self::sync`] writes
    /// the full state (after entering, re-attaching or switching panes).
    pub fn invalidate(&mut self) {
        self.applied = None;
        self.title = None;
    }

    /// Brings the host in line with `modes` and `cursor`, writing only what
    /// changed since the last call.
    pub fn sync(&mut self, modes: &TerminalModes, cursor: &Cursor, out: &mut Vec<u8>) {
        let next = Applied {
            flags: host_flags(modes, self.mouse),
            kitty_flags: modes.kitty_flags,
            cursor: (cursor.shape, cursor.blinking),
        };
        let prev = self.applied.take();
        let changed = prev.as_ref().map(|p| p.flags ^ next.flags);
        write_dec_modes(next.flags, changed, out);
        if prev
            .as_ref()
            .is_none_or(|p| p.kitty_flags != next.kitty_flags)
            && self.kitty
        {
            out.extend_from_slice(format!("\x1b[={};1u", next.kitty_flags).as_bytes());
        }
        if prev.as_ref().is_none_or(|p| p.cursor != next.cursor) {
            let (shape, blinking) = next.cursor;
            out.extend_from_slice(format!("\x1b[{} q", shape.decscusr(blinking)).as_bytes());
        }
        self.applied = Some(next);
    }

    /// Sets the host title (OSC 2) when it changed; `None` clears it so the
    /// host shows its default. Control characters are dropped.
    pub fn sync_title(&mut self, title: Option<&str>, out: &mut Vec<u8>) {
        let title = title.unwrap_or_default();
        if self.title.as_deref() == Some(title) {
            return;
        }
        let clean: String = title.chars().filter(|c| !c.is_control()).collect();
        out.extend_from_slice(format!("\x1b]2;{clean}\x1b\\").as_bytes());
        self.title = Some(title.to_owned());
    }
}

/// The host-side view of a pane's modes. Alternate scroll only applies to
/// the alternate screen, but the host is always on its alternate screen, so
/// it is mirrored only while the pane is too.
fn host_flags(modes: &TerminalModes, mouse: Option<MouseMode>) -> ModeFlags {
    let mut flags = modes.flags;
    if !flags.contains(ModeFlags::ALT_SCREEN) {
        flags.remove(ModeFlags::ALTERNATE_SCROLL);
    }
    if let Some(policy) = mouse {
        flags = captured(flags, policy);
    }
    flags & (mirrored() | ModeFlags::APP_KEYPAD)
}

/// Mouse modes when Monarch captures: `auto` always tracks drags (and all
/// motion if the pane wants it), `pane` tracks what the pane asked for,
/// `off` never tracks. Reports are SGR whenever tracking is on, and the
/// client then handles the wheel itself, so the host's alternate scroll
/// is turned off.
fn captured(flags: ModeFlags, policy: MouseMode) -> ModeFlags {
    let tracking = ModeFlags::MOUSE_CLICK | ModeFlags::MOUSE_DRAG | ModeFlags::MOUSE_MOTION;
    let wanted = flags & tracking;
    let capture = match policy {
        MouseMode::Auto => ModeFlags::MOUSE_DRAG | (wanted & ModeFlags::MOUSE_MOTION),
        MouseMode::Pane => wanted,
        MouseMode::Off => ModeFlags::empty(),
    };
    let mut flags = flags - (tracking | ModeFlags::UTF8_MOUSE | ModeFlags::SGR_MOUSE);
    if !capture.is_empty() {
        flags |= capture | ModeFlags::SGR_MOUSE;
        flags.remove(ModeFlags::ALTERNATE_SCROLL);
    }
    flags
}

fn mirrored() -> ModeFlags {
    DEC_MODES
        .iter()
        .fold(ModeFlags::empty(), |all, (flag, _)| all | *flag)
}

/// Resets before sets: xterm's mouse modes are exclusive, and turning one
/// off after turning another on would switch the mouse off entirely.
fn write_dec_modes(flags: ModeFlags, changed: Option<ModeFlags>, out: &mut Vec<u8>) {
    let changed = changed.unwrap_or(ModeFlags::all());
    for on in [false, true] {
        for (flag, mode) in DEC_MODES {
            if changed.contains(flag) && flags.contains(flag) == on {
                dec(out, mode, on);
            }
        }
    }
    if changed.contains(ModeFlags::APP_KEYPAD) {
        let keypad = flags.contains(ModeFlags::APP_KEYPAD);
        out.extend_from_slice(if keypad { b"\x1b=" } else { b"\x1b>" });
    }
}

fn dec(out: &mut Vec<u8>, mode: u16, on: bool) {
    let action = if on { 'h' } else { 'l' };
    out.extend_from_slice(format!("\x1b[?{mode}{action}").as_bytes());
}

#[cfg(test)]
mod tests;
