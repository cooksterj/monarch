//! Converts alacritty's `TermMode` into the modes the client mirrors.

use alacritty_terminal::term::TermMode;
use monarch_proto::{ModeFlags, TerminalModes};

/// alacritty mode → Monarch mode, declared once as a table.
const MODES: [(TermMode, ModeFlags); 13] = [
    (TermMode::SHOW_CURSOR, ModeFlags::SHOW_CURSOR),
    (TermMode::APP_CURSOR, ModeFlags::APP_CURSOR),
    (TermMode::APP_KEYPAD, ModeFlags::APP_KEYPAD),
    (TermMode::ALT_SCREEN, ModeFlags::ALT_SCREEN),
    (TermMode::BRACKETED_PASTE, ModeFlags::BRACKETED_PASTE),
    (TermMode::FOCUS_IN_OUT, ModeFlags::FOCUS_IN_OUT),
    (TermMode::MOUSE_REPORT_CLICK, ModeFlags::MOUSE_CLICK),
    (TermMode::MOUSE_DRAG, ModeFlags::MOUSE_DRAG),
    (TermMode::MOUSE_MOTION, ModeFlags::MOUSE_MOTION),
    (TermMode::SGR_MOUSE, ModeFlags::SGR_MOUSE),
    (TermMode::UTF8_MOUSE, ModeFlags::UTF8_MOUSE),
    (TermMode::ALTERNATE_SCROLL, ModeFlags::ALTERNATE_SCROLL),
    (TermMode::LINE_WRAP, ModeFlags::LINE_WRAP),
];

/// Kitty keyboard flags in protocol bit order (1, 2, 4, 8, 16).
const KITTY: [TermMode; 5] = [
    TermMode::DISAMBIGUATE_ESC_CODES,
    TermMode::REPORT_EVENT_TYPES,
    TermMode::REPORT_ALTERNATE_KEYS,
    TermMode::REPORT_ALL_KEYS_AS_ESC,
    TermMode::REPORT_ASSOCIATED_TEXT,
];

/// Stateless mode conversion.
pub(crate) struct ModeConverter;

impl ModeConverter {
    /// Converts a full mode set.
    pub(crate) fn modes(mode: TermMode) -> TerminalModes {
        let flags = MODES
            .iter()
            .filter(|(m, _)| mode.contains(*m))
            .fold(ModeFlags::empty(), |acc, (_, f)| acc | *f);
        let kitty_flags = KITTY
            .iter()
            .enumerate()
            .filter(|(_, m)| mode.contains(**m))
            .fold(0u8, |acc, (bit, _)| acc | (1 << bit));
        TerminalModes { flags, kitty_flags }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_dec_modes() {
        let m = ModeConverter::modes(TermMode::SHOW_CURSOR | TermMode::BRACKETED_PASTE);
        assert_eq!(m.flags, ModeFlags::SHOW_CURSOR | ModeFlags::BRACKETED_PASTE);
        assert_eq!(m.kitty_flags, 0);
    }

    #[test]
    fn kitty_flags_use_protocol_bits() {
        let m = ModeConverter::modes(
            TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_ALL_KEYS_AS_ESC,
        );
        assert_eq!(m.kitty_flags, 1 | 8);
    }

    #[test]
    fn empty_mode_is_empty() {
        let m = ModeConverter::modes(TermMode::empty());
        assert!(m.flags.is_empty());
    }
}
