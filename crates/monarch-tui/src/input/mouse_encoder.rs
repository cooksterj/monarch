//! Encodes a mouse event the way a pane asked for it (G3).
//!
//! The host always reports in SGR; panes may want SGR (1006), UTF-8 (1005)
//! or the original X10 bytes, and only the events their tracking level
//! (1000 clicks, 1002 drags, 1003 all motion) covers.

use monarch_proto::ModeFlags;

use super::{MouseButton, MouseEvent, MouseKind, WheelDirection};

const MOTION: u8 = 32;
const WHEEL: u8 = 64;
const LEGACY_RELEASE: u8 = 3;
/// X10 adds 32 to a value and sends one byte.
const X10_MAX: u16 = 255 - 32;
/// UTF-8 mode extends coordinates to 2015 (xterm's limit).
const UTF8_MAX: u16 = 2015;

/// Stateless encoder from [`MouseEvent`] to pane bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseEncoder;

impl MouseEncoder {
    /// The bytes for `event` at pane cell (`col`, `row`), or `None` when the
    /// pane's modes do not ask for this event or the position cannot be
    /// encoded.
    #[must_use]
    pub fn encode(event: &MouseEvent, modes: ModeFlags, col: u16, row: u16) -> Option<Vec<u8>> {
        if !wants(event.kind, modes) {
            return None;
        }
        let code = code(event.kind) | event.mods;
        let (x, y) = (col.checked_add(1)?, row.checked_add(1)?);
        if modes.contains(ModeFlags::SGR_MOUSE) {
            let fin = if matches!(event.kind, MouseKind::Release(_)) {
                'm'
            } else {
                'M'
            };
            return Some(format!("\x1b[<{code};{x};{y}{fin}").into_bytes());
        }
        let code = if matches!(event.kind, MouseKind::Release(_)) {
            LEGACY_RELEASE | event.mods
        } else {
            code
        };
        let mut out = b"\x1b[M".to_vec();
        out.push(32 + code);
        if modes.contains(ModeFlags::UTF8_MOUSE) {
            for v in [x, y] {
                let c = char::from_u32(u32::from(v.min(UTF8_MAX)) + 32)?;
                out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
            }
        } else {
            if x > X10_MAX || y > X10_MAX {
                return None;
            }
            out.extend([32 + x as u8, 32 + y as u8]);
        }
        Some(out)
    }
}

/// Whether the pane's tracking level covers this kind of event.
fn wants(kind: MouseKind, modes: ModeFlags) -> bool {
    match kind {
        MouseKind::Motion => modes.contains(ModeFlags::MOUSE_MOTION),
        MouseKind::Drag(_) => modes.intersects(ModeFlags::MOUSE_DRAG | ModeFlags::MOUSE_MOTION),
        _ => modes.intersects(ModeFlags::ANY_MOUSE),
    }
}

fn code(kind: MouseKind) -> u8 {
    let button = |b: MouseButton| match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    match kind {
        MouseKind::Press(b) | MouseKind::Release(b) => button(b),
        MouseKind::Drag(b) => button(b) | MOTION,
        MouseKind::Motion => LEGACY_RELEASE | MOTION,
        MouseKind::Wheel(d) => {
            WHEEL
                | match d {
                    WheelDirection::Up => 0,
                    WheelDirection::Down => 1,
                    WheelDirection::Left => 2,
                    WheelDirection::Right => 3,
                }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: MouseKind, mods: u8) -> MouseEvent {
        MouseEvent {
            kind,
            col: 0,
            row: 0,
            mods,
        }
    }

    const LEFT: MouseKind = MouseKind::Press(MouseButton::Left);

    #[test]
    fn sgr_panes_get_sgr_with_pane_coordinates() {
        let sgr = ModeFlags::MOUSE_CLICK | ModeFlags::SGR_MOUSE;
        assert_eq!(
            MouseEncoder::encode(&ev(LEFT, 0), sgr, 4, 9),
            Some(b"\x1b[<0;5;10M".to_vec())
        );
        let up = ev(MouseKind::Release(MouseButton::Right), 4);
        assert_eq!(
            MouseEncoder::encode(&up, sgr, 0, 0),
            Some(b"\x1b[<6;1;1m".to_vec())
        );
        let wheel = ev(MouseKind::Wheel(WheelDirection::Down), 0);
        assert_eq!(
            MouseEncoder::encode(&wheel, sgr, 0, 0),
            Some(b"\x1b[<65;1;1M".to_vec())
        );
    }

    #[test]
    fn legacy_panes_get_x10_bytes_and_generic_release() {
        let x10 = ModeFlags::MOUSE_CLICK;
        assert_eq!(
            MouseEncoder::encode(&ev(LEFT, 0), x10, 0, 0),
            Some(b"\x1b[M !!".to_vec())
        );
        let up = ev(MouseKind::Release(MouseButton::Left), 0);
        assert_eq!(
            MouseEncoder::encode(&up, x10, 0, 0),
            Some(b"\x1b[M#!!".to_vec())
        );
        assert_eq!(
            MouseEncoder::encode(&ev(LEFT, 0), x10, 300, 0),
            None,
            "beyond X10 range"
        );
    }

    #[test]
    fn utf8_panes_reach_past_column_223() {
        let utf8 = ModeFlags::MOUSE_CLICK | ModeFlags::UTF8_MOUSE;
        let bytes = MouseEncoder::encode(&ev(LEFT, 0), utf8, 299, 0).expect("encodes");
        assert_eq!(bytes, "\x1b[M \u{14c}!".as_bytes());
    }

    #[test]
    fn tracking_level_filters_drags_and_motion() {
        let drag = ev(MouseKind::Drag(MouseButton::Left), 0);
        let motion = ev(MouseKind::Motion, 0);
        let sgr = ModeFlags::SGR_MOUSE;
        assert_eq!(
            MouseEncoder::encode(&drag, sgr | ModeFlags::MOUSE_CLICK, 0, 0),
            None
        );
        assert_eq!(
            MouseEncoder::encode(&drag, sgr | ModeFlags::MOUSE_DRAG, 0, 0),
            Some(b"\x1b[<32;1;1M".to_vec())
        );
        assert_eq!(
            MouseEncoder::encode(&motion, sgr | ModeFlags::MOUSE_DRAG, 0, 0),
            None
        );
        assert_eq!(
            MouseEncoder::encode(&motion, sgr | ModeFlags::MOUSE_MOTION, 0, 0),
            Some(b"\x1b[<35;1;1M".to_vec())
        );
        assert_eq!(
            MouseEncoder::encode(&ev(LEFT, 0), sgr, 0, 0),
            None,
            "no tracking mode"
        );
    }
}
