//! Decides what a host mouse event means for the focused pane (G3).
//!
//! A pane that enabled mouse tracking gets the event in its own encoding,
//! with pane-local coordinates; a press that started inside keeps sending
//! drags and the release even if the pointer leaves. Without tracking, the
//! wheel scrolls history on the primary screen and becomes arrow keys on
//! the alternate screen when the pane allows it (1007), as in a bare
//! terminal. The left button without tracking, or with Shift held over a
//! tracking pane, selects text (tmux's convention); the right button the
//! same way opens the context menu.

use std::time::Instant;

use monarch_proto::ModeFlags;

use super::{ClickCounter, FocusView, MouseAction, SelectEvent};
use crate::{MouseButton, MouseEncoder, MouseEvent, MouseKind, WheelDirection};

/// xterm's Shift bit in a button code.
const SHIFT: u8 = 4;

/// Lines per wheel notch, as xterm and most terminals scroll.
pub const WHEEL_LINES: i32 = 3;

/// Routing state: whether a button press went to the pane or started a
/// selection, where it started, and recent clicks.
#[derive(Debug, Clone, Copy, Default)]
pub struct MouseRouter {
    captured: bool,
    selecting: bool,
    /// Host row of the press that started the selection.
    press_row: u16,
    clicks: ClickCounter,
}

impl MouseRouter {
    /// A router with no button held.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Routes one event for the focused pane described by `view`.
    pub fn route(&mut self, event: &MouseEvent, view: &FocusView, now: Instant) -> MouseAction {
        if view.pane.is_none() {
            return MouseAction::Ignore;
        }
        let local = view.local(event.col, event.row);
        let tracking = view.modes.intersects(ModeFlags::ANY_MOUSE);
        let shift = event.mods & SHIFT != 0;
        let left_press = matches!(event.kind, MouseKind::Press(MouseButton::Left));
        let right_press = matches!(event.kind, MouseKind::Press(MouseButton::Right));
        if right_press && !self.selecting && local.is_some() && (!tracking || shift) {
            return MouseAction::Menu {
                col: event.col,
                row: event.row,
            };
        }
        if self.selecting || (left_press && local.is_some() && (!tracking || shift)) {
            return self.select(event, view, now);
        }
        if tracking {
            return self.route_to_pane(event, view, local);
        }
        match (event.kind, local) {
            (MouseKind::Wheel(dir), Some(_)) => wheel(dir, view.modes),
            _ => MouseAction::Ignore,
        }
    }

    /// Left press, drag and release while selecting.
    fn select(&mut self, event: &MouseEvent, view: &FocusView, now: Instant) -> MouseAction {
        let (col, row) = view.clamped(event.col, event.row);
        let gesture = match event.kind {
            MouseKind::Press(MouseButton::Left) => {
                self.selecting = true;
                self.press_row = event.row;
                let clicks = self.clicks.press(col, row, now);
                SelectEvent::Start { col, row, clicks }
            }
            MouseKind::Drag(MouseButton::Left) => SelectEvent::Extend {
                col,
                row,
                beyond: self.beyond(view, event.row),
            },
            MouseKind::Release(MouseButton::Left) => {
                self.selecting = false;
                SelectEvent::End
            }
            _ => return MouseAction::Ignore,
        };
        MouseAction::Select(gesture)
    }

    /// Rows host row `row` is past the focused pane's top (negative) or
    /// bottom (positive). The pointer cannot go above the host's top row,
    /// so on a pane touching it that row counts as one past once the drag
    /// has left the row it started on.
    fn beyond(&self, view: &FocusView, row: u16) -> i16 {
        let (row, top) = (i32::from(row), i32::from(view.y));
        let bottom = top + i32::from(view.rows) - 1;
        let past = if row < top {
            row - top
        } else if row > bottom {
            row - bottom
        } else if row == 0 && self.press_row > 0 {
            -1
        } else {
            0
        };
        i16::try_from(past).unwrap_or(0)
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

    impl MouseRouter {
        fn route_now(&mut self, event: &MouseEvent, view: &FocusView) -> MouseAction {
            self.route(event, view, Instant::now())
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
            r.route_now(&ev(UP, 5, 5), &view(ModeFlags::empty())),
            MouseAction::Scroll(3)
        );
        assert_eq!(
            r.route_now(&ev(DOWN, 5, 5), &view(ModeFlags::empty())),
            MouseAction::Scroll(-3)
        );
        assert_eq!(
            r.route_now(&ev(UP, 5, 23), &view(ModeFlags::empty())),
            MouseAction::Ignore,
            "status row"
        );
    }

    #[test]
    fn wheel_becomes_arrows_on_the_alternate_screen() {
        let mut r = MouseRouter::new();
        let alt = ModeFlags::ALT_SCREEN | ModeFlags::ALTERNATE_SCROLL;
        assert_eq!(
            r.route_now(&ev(UP, 0, 0), &view(alt)),
            MouseAction::Forward(b"\x1b[A\x1b[A\x1b[A".to_vec())
        );
        let app = alt | ModeFlags::APP_CURSOR;
        assert_eq!(
            r.route_now(&ev(DOWN, 0, 0), &view(app)),
            MouseAction::Forward(b"\x1bOB\x1bOB\x1bOB".to_vec())
        );
        assert_eq!(
            r.route_now(&ev(UP, 0, 0), &view(ModeFlags::ALT_SCREEN)),
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
            r.route_now(&ev(MouseKind::Press(left), 12, 3), &v),
            MouseAction::Forward(b"\x1b[<0;3;3M".to_vec())
        );
        assert_eq!(
            r.route_now(&ev(MouseKind::Drag(left), 50, 30), &v),
            MouseAction::Forward(b"\x1b[<32;20;10M".to_vec())
        );
        assert_eq!(
            r.route_now(&ev(MouseKind::Release(left), 0, 0), &v),
            MouseAction::Forward(b"\x1b[<0;1;1m".to_vec())
        );
        assert_eq!(
            r.route_now(&ev(MouseKind::Drag(left), 0, 0), &v),
            MouseAction::Ignore,
            "no press in progress"
        );
        assert_eq!(
            r.route_now(&ev(MouseKind::Press(left), 0, 0), &v),
            MouseAction::Ignore,
            "outside"
        );
    }

    #[test]
    fn right_clicks_open_the_menu_unless_the_pane_wants_them() {
        let right = MouseKind::Press(MouseButton::Right);
        let mut r = MouseRouter::new();
        let plain = view(ModeFlags::empty());
        assert_eq!(
            r.route_now(&ev(right, 4, 2), &plain),
            MouseAction::Menu { col: 4, row: 2 }
        );
        let tracking = view(ModeFlags::MOUSE_CLICK | ModeFlags::SGR_MOUSE);
        assert!(matches!(
            r.route_now(&ev(right, 4, 2), &tracking),
            MouseAction::Forward(_)
        ));
        let shifted = MouseEvent {
            mods: SHIFT,
            ..ev(right, 4, 2)
        };
        assert_eq!(
            r.route_now(&shifted, &tracking),
            MouseAction::Menu { col: 4, row: 2 },
            "Shift reaches Monarch over a tracking pane"
        );
        assert_eq!(r.route_now(&ev(right, 90, 40), &plain), MouseAction::Ignore);
    }

    #[test]
    fn drags_past_an_edge_say_how_far() {
        let left = MouseButton::Left;
        let beyond = |press_row, drag_row, v: &FocusView| {
            let mut r = MouseRouter::new();
            r.route_now(&ev(MouseKind::Press(left), 2, press_row), v);
            match r.route_now(&ev(MouseKind::Drag(left), 2, drag_row), v) {
                MouseAction::Select(SelectEvent::Extend { beyond, .. }) => beyond,
                other => panic!("not a drag: {other:?}"),
            }
        };
        let top = view(ModeFlags::empty());
        assert_eq!(beyond(5, 3, &top), 0, "inside");
        assert_eq!(beyond(5, 0, &top), -1, "the host's top row");
        assert_eq!(beyond(0, 0, &top), 0, "a drag along the top row");
        assert_eq!(beyond(5, 23, &top), 1, "the status line");
        let lower = FocusView {
            y: 10,
            rows: 5,
            ..top
        };
        assert_eq!(beyond(12, 7, &lower), -3);
        assert_eq!(beyond(12, 12, &lower), 0);
    }

    #[test]
    fn plain_panes_select_with_the_left_button() {
        let mut r = MouseRouter::new();
        let v = view(ModeFlags::empty());
        let left = MouseButton::Left;
        let t = Instant::now();
        assert_eq!(
            r.route(&ev(MouseKind::Press(left), 2, 1), &v, t),
            MouseAction::Select(SelectEvent::Start {
                col: 2,
                row: 1,
                clicks: 1
            })
        );
        assert_eq!(
            r.route(&ev(MouseKind::Drag(left), 90, 40), &v, t),
            MouseAction::Select(SelectEvent::Extend {
                col: 79,
                row: 22,
                beyond: 18
            })
        );
        assert_eq!(
            r.route(&ev(MouseKind::Release(left), 90, 40), &v, t),
            MouseAction::Select(SelectEvent::End)
        );
        let again = r.route(&ev(MouseKind::Press(left), 2, 1), &v, t);
        assert_eq!(
            again,
            MouseAction::Select(SelectEvent::Start {
                col: 2,
                row: 1,
                clicks: 2
            }),
            "double click"
        );
    }

    #[test]
    fn shift_selects_over_a_tracking_pane() {
        let mut r = MouseRouter::new();
        let v = view(ModeFlags::MOUSE_CLICK | ModeFlags::SGR_MOUSE);
        let press = MouseEvent {
            mods: SHIFT,
            ..ev(MouseKind::Press(MouseButton::Left), 0, 0)
        };
        assert!(matches!(
            r.route(&press, &v, Instant::now()),
            MouseAction::Select(_)
        ));
        let plain = ev(MouseKind::Release(MouseButton::Left), 0, 0);
        assert_eq!(
            r.route(&plain, &v, Instant::now()),
            MouseAction::Select(SelectEvent::End),
            "release ends it"
        );
    }

    #[test]
    fn no_pane_means_nothing() {
        let mut r = MouseRouter::new();
        let v = FocusView {
            pane: None,
            ..view(ModeFlags::empty())
        };
        assert_eq!(r.route_now(&ev(UP, 0, 0), &v), MouseAction::Ignore);
    }
}
