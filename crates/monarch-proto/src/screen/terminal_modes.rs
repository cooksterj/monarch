//! The complete set of mirrored modes: DEC flags plus kitty keyboard flags.

use serde::{Deserialize, Serialize};

use super::ModeFlags;

/// Modes a pane has enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TerminalModes {
    /// DEC/xterm private modes.
    pub flags: ModeFlags,
    /// Kitty keyboard progressive-enhancement flags (`CSI = flags u`):
    /// 1 disambiguate, 2 event types, 4 alternate keys, 8 all keys as
    /// escapes, 16 associated text.
    pub kitty_flags: u8,
}

impl Default for TerminalModes {
    fn default() -> Self {
        Self {
            flags: ModeFlags::SHOW_CURSOR | ModeFlags::LINE_WRAP,
            kitty_flags: 0,
        }
    }
}

impl TerminalModes {
    /// Whether the program wants mouse events.
    #[must_use]
    pub fn wants_mouse(&self) -> bool {
        self.flags.intersects(ModeFlags::ANY_MOUSE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_a_fresh_terminal() {
        let m = TerminalModes::default();
        assert!(m.flags.contains(ModeFlags::SHOW_CURSOR));
        assert!(!m.wants_mouse());
        assert_eq!(m.kitty_flags, 0);
    }

    #[test]
    fn wants_mouse_when_any_reporting_mode_is_on() {
        let m = TerminalModes {
            flags: ModeFlags::MOUSE_CLICK | ModeFlags::SGR_MOUSE,
            kitty_flags: 0,
        };
        assert!(m.wants_mouse());
    }
}
