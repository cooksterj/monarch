//! Decides what a host mouse event means for the focused pane (G3).
//!
//! A pane that enabled mouse tracking gets the event in its own encoding,
//! with pane-local coordinates; a press that started inside keeps sending
//! drags and the release even if the pointer leaves. Without tracking, the
//! wheel scrolls history on the primary screen and becomes arrow keys on
//! the alternate screen when the pane allows it (1007), as in a bare
//! terminal. Clicks without tracking are for selection (not yet built).

use monarch_proto::ModeFlags;

use super::{FocusView, MouseAction};
use crate::{MouseEncoder, MouseEvent, MouseKind, WheelDirection};

/// Lines per wheel notch, as xterm and most terminals scroll.
pub const WHEEL_LINES: i32 = 3;

/// Routing state: whether a button press went to the pane.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseRouter {
    captured: bool,
}

impl MouseRouter {
    /// A router with no button held.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Routes one event for the focused pane described by `view`.
    pub fn route(&mut self, event: &MouseEvent, view: &FocusView) -> MouseAction {
        if view.pane.is_none() {
            return MouseAction::Ignore;
        }
        let local = view.local(event.col, event.row);
        if view.modes.intersects(ModeFlags::ANY_MOUSE) {
            return self.route_to_pane(event, view, local);
        }
        match (event.kind, local) {
            (MouseKind::Wheel(dir), Some(_)) => wheel(dir, view.modes),
            _ => MouseAction::Ignore,
        }
    }

    fn route_to_pane(
        &mut self,
        event: &MouseEvent,
        view: &FocusView,
        local: Option<(u16, u16)>,
    ) -> MouseAction {
        let (col, row) = match (event.kind, local) {
            (_, Some(at)) => at,
            (MouseKind::Drag(_) | MouseKind::Release(_), None) if self.captured => {
                view.clamped(event.col, event.row)
            }
            _ => return MouseAction::Ignore,
        };
        match event.kind {
            MouseKind::Press(_) => self.captured = true,
            MouseKind::Release(_) => self.captured = false,
            _ => {}
        }
        MouseEncoder::encode(event, view.modes, col, row)
            .map_or(MouseAction::Ignore, MouseAction::Forward)
    }
}

/// The wheel without mouse tracking.
fn wheel(dir: WheelDirection, modes: ModeFlags) -> MouseAction {
    let up = match dir {
        WheelDirection::Up => true,
        WheelDirection::Down => false,
        WheelDirection::Left | WheelDirection::Right => return MouseAction::Ignore,
    };
    if !modes.contains(ModeFlags::ALT_SCREEN) {
        return MouseAction::Scroll(if up { WHEEL_LINES } else { -WHEEL_LINES });
    }
    if !modes.contains(ModeFlags::ALTERNATE_SCROLL) {
        return MouseAction::Ignore;
    }
    let intro: &[u8] = if modes.contains(ModeFlags::APP_CURSOR) {
        b"\x1bO"
    } else {
        b"\x1b["
    };
    let key = if up { b'A' } else { b'B' };
    let mut bytes = Vec::new();
    for _ in 0..WHEEL_LINES {
        bytes.extend_from_slice(intro);
        bytes.push(key);
    }
    MouseAction::Forward(bytes)
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;

    use super::*;
    use crate::MouseButton;

    fn view(modes: ModeFlags) -> FocusView {
        FocusView {
            pane: Some(PaneId(1)),
            x: 0,
            y: 0,
            cols: 80,
            rows: 23,
            modes,
            scrolled: false,
        }
    }

    fn ev(kind: MouseKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            col,
            row,
            mods: 0,
        }
    }

    const UP: MouseKind = MouseKind::Wheel(WheelDirection::Up);
    const DOWN: MouseKind = MouseKind::Wheel(WheelDirection::Down);

    #[test]
    fn wheel_scrolls_history_on_the_primary_screen() {
        let mut r = MouseRouter::new();
        assert_eq!(
            r.route(&ev(UP, 5, 5), &view(ModeFlags::empty())),
            MouseAction::Scroll(3)
        );
        assert_eq!(
            r.route(&ev(DOWN, 5, 5), &view(ModeFlags::empty())),
            MouseAction::Scroll(-3)
        );
        assert_eq!(
            r.route(&ev(UP, 5, 23), &view(ModeFlags::empty())),
            MouseAction::Ignore,
            "status row"
        );
    }

    #[test]
    fn wheel_becomes_arrows_on_the_alternate_screen() {
        let mut r = MouseRouter::new();
        let alt = ModeFlags::ALT_SCREEN | ModeFlags::ALTERNATE_SCROLL;
        assert_eq!(
            r.route(&ev(UP, 0, 0), &view(alt)),
            MouseAction::Forward(b"\x1b[A\x1b[A\x1b[A".to_vec())
        );
        let app = alt | ModeFlags::APP_CURSOR;
        assert_eq!(
            r.route(&ev(DOWN, 0, 0), &view(app)),
            MouseAction::Forward(b"\x1bOB\x1bOB\x1bOB".to_vec())
        );
        assert_eq!(
            r.route(&ev(UP, 0, 0), &view(ModeFlags::ALT_SCREEN)),
            MouseAction::Ignore
        );
    }

    #[test]
    fn tracking_panes_get_encoded_events_and_keep_their_drag() {
        let mut r = MouseRouter::new();
        let v = FocusView {
            x: 10,
            y: 1,
            cols: 20,
            rows: 10,
            ..view(ModeFlags::MOUSE_DRAG | ModeFlags::SGR_MOUSE)
        };
        let left = MouseButton::Left;
        assert_eq!(
            r.route(&ev(MouseKind::Press(left), 12, 3), &v),
            MouseAction::Forward(b"\x1b[<0;3;3M".to_vec())
        );
        assert_eq!(
            r.route(&ev(MouseKind::Drag(left), 50, 30), &v),
            MouseAction::Forward(b"\x1b[<32;20;10M".to_vec())
        );
        assert_eq!(
            r.route(&ev(MouseKind::Release(left), 0, 0), &v),
            MouseAction::Forward(b"\x1b[<0;1;1m".to_vec())
        );
        assert_eq!(
            r.route(&ev(MouseKind::Drag(left), 0, 0), &v),
            MouseAction::Ignore,
            "no press in progress"
        );
        assert_eq!(
            r.route(&ev(MouseKind::Press(left), 0, 0), &v),
            MouseAction::Ignore,
            "outside"
        );
    }

    #[test]
    fn no_pane_means_nothing() {
        let mut r = MouseRouter::new();
        let v = FocusView {
            pane: None,
            ..view(ModeFlags::empty())
        };
        assert_eq!(r.route(&ev(UP, 0, 0), &v), MouseAction::Ignore);
    }
}
