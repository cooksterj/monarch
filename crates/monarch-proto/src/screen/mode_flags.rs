//! Terminal modes that the client must mirror onto the host terminal.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    /// DEC/xterm private modes set by the program inside a pane. These are
    /// exactly the modes the TUI mirrors to the host when the pane has focus
    /// (Terminal Fidelity Contract G2–G7).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct ModeFlags: u32 {
        /// DECTCEM: cursor visible.
        const SHOW_CURSOR = 1 << 0;
        /// DECCKM: application cursor keys.
        const APP_CURSOR = 1 << 1;
        /// DECKPAM: application keypad.
        const APP_KEYPAD = 1 << 2;
        /// 1049/47: alternate screen.
        const ALT_SCREEN = 1 << 3;
        /// 2004: bracketed paste.
        const BRACKETED_PASTE = 1 << 4;
        /// 1004: focus in/out reporting.
        const FOCUS_IN_OUT = 1 << 5;
        /// 1000: report button presses.
        const MOUSE_CLICK = 1 << 6;
        /// 1002: report drags.
        const MOUSE_DRAG = 1 << 7;
        /// 1003: report all motion.
        const MOUSE_MOTION = 1 << 8;
        /// 1006: SGR mouse encoding.
        const SGR_MOUSE = 1 << 9;
        /// 1005: UTF-8 mouse encoding.
        const UTF8_MOUSE = 1 << 10;
        /// 1007: wheel sends arrow keys on the alternate screen.
        const ALTERNATE_SCROLL = 1 << 11;
        /// DECAWM: line wrap.
        const LINE_WRAP = 1 << 12;
        /// Any mouse reporting mode.
        const ANY_MOUSE = Self::MOUSE_CLICK.bits() | Self::MOUSE_DRAG.bits()
            | Self::MOUSE_MOTION.bits();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_mouse_excludes_encodings() {
        assert!(ModeFlags::ANY_MOUSE.contains(ModeFlags::MOUSE_DRAG));
        assert!(!ModeFlags::ANY_MOUSE.contains(ModeFlags::SGR_MOUSE));
    }
}
