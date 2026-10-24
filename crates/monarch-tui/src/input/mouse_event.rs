//! A mouse report from the host terminal, decoded from SGR (1006) form.

/// Which button a press, release or drag concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Button 1.
    Left,
    /// Button 2.
    Middle,
    /// Button 3.
    Right,
}

/// Wheel direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelDirection {
    /// Towards history.
    Up,
    /// Towards live output.
    Down,
    /// Horizontal, left.
    Left,
    /// Horizontal, right.
    Right,
}

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseKind {
    /// A button went down.
    Press(MouseButton),
    /// A button came up.
    Release(MouseButton),
    /// The pointer moved with a button held.
    Drag(MouseButton),
    /// The pointer moved with no button held.
    Motion,
    /// One wheel notch.
    Wheel(WheelDirection),
}

/// One decoded mouse report in host cell coordinates (0-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    /// What happened.
    pub kind: MouseKind,
    /// Host column, from 0.
    pub col: u16,
    /// Host row, from 0.
    pub row: u16,
    /// Modifier bits as xterm encodes them: shift 4, alt 8, ctrl 16.
    pub mods: u8,
}

/// The modifier bits of an xterm button code.
pub const MOD_BITS: u8 = 4 | 8 | 16;
const MOTION: u8 = 32;
const WHEEL: u8 = 64;
const EXTRA: u8 = 128;

impl MouseEvent {
    /// Decodes the parameters of `CSI < Cb ; Cx ; Cy M|m`. `release` is
    /// true for the `m` final. Extra buttons (8–11) and malformed
    /// parameters give `None`.
    #[must_use]
    pub fn from_sgr(params: &[u8], release: bool) -> Option<Self> {
        let text = std::str::from_utf8(params).ok()?;
        let mut parts = text.split(';').map(str::parse::<u16>);
        let (code, col, row) = (
            parts.next()?.ok()?,
            parts.next()?.ok()?,
            parts.next()?.ok()?,
        );
        if parts.next().is_some() || col == 0 || row == 0 {
            return None;
        }
        let code = u8::try_from(code).ok()?;
        let kind = Self::kind(code, release)?;
        Some(Self {
            kind,
            col: col - 1,
            row: row - 1,
            mods: code & MOD_BITS,
        })
    }

    fn kind(code: u8, release: bool) -> Option<MouseKind> {
        if code & EXTRA != 0 {
            return None;
        }
        let button = match code & 3 {
            0 => Some(MouseButton::Left),
            1 => Some(MouseButton::Middle),
            2 => Some(MouseButton::Right),
            _ => None,
        };
        if code & WHEEL != 0 {
            let dir = [
                WheelDirection::Up,
                WheelDirection::Down,
                WheelDirection::Left,
                WheelDirection::Right,
            ];
            return Some(MouseKind::Wheel(dir[usize::from(code & 3)]));
        }
        Some(match (code & MOTION != 0, button) {
            (true, Some(b)) => MouseKind::Drag(b),
            (true, None) => MouseKind::Motion,
            (false, Some(b)) if release => MouseKind::Release(b),
            (false, Some(b)) => MouseKind::Press(b),
            (false, None) => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(params: &str, release: bool) -> Option<MouseEvent> {
        MouseEvent::from_sgr(params.as_bytes(), release)
    }

    #[test]
    fn presses_releases_drags_and_motion() {
        let press = ev("0;5;3", false).expect("press");
        assert_eq!(
            (press.kind, press.col, press.row),
            (MouseKind::Press(MouseButton::Left), 4, 2)
        );
        assert_eq!(
            ev("2;1;1", true).map(|e| e.kind),
            Some(MouseKind::Release(MouseButton::Right))
        );
        assert_eq!(
            ev("33;1;1", false).map(|e| e.kind),
            Some(MouseKind::Drag(MouseButton::Middle))
        );
        assert_eq!(ev("35;1;1", false).map(|e| e.kind), Some(MouseKind::Motion));
    }

    #[test]
    fn wheel_and_modifiers() {
        let up = ev("64;1;1", false).expect("wheel");
        assert_eq!(up.kind, MouseKind::Wheel(WheelDirection::Up));
        assert_eq!(
            ev("65;1;1", false).map(|e| e.kind),
            Some(MouseKind::Wheel(WheelDirection::Down))
        );
        assert_eq!(
            ev("20;1;1", false).map(|e| e.mods),
            Some(4 | 16),
            "shift+ctrl click"
        );
    }

    #[test]
    fn malformed_and_unsupported_reports_are_rejected() {
        for bad in [
            "", "0;1", "0;1;1;1", "x;1;1", "0;0;1", "128;1;1", "3;1;1", "999;1;1",
        ] {
            assert_eq!(ev(bad, false), None, "{bad}");
        }
    }
}
