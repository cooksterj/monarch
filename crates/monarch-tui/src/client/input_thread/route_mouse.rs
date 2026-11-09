//! Where a host mouse event goes: a divider drag, the sidebar's wheel, a
//! click that focuses another pane, or the router for the focused pane.

use std::time::Instant;

use monarch_proto::ClientMsg;

use super::{InputParams, Pipeline};
use crate::{
    ClientEvent, DividerGrab, MOD_BITS, MouseAction, MouseButton, MouseEvent, MouseKind,
    WHEEL_LINES, WheelDirection,
};

/// A press on another pane focuses it (and nothing else); everything
/// else goes through the router for the focused pane.
pub(super) fn route_mouse(p: &InputParams, pipe: &mut Pipeline, event: &MouseEvent) {
    if pipe.focusing {
        pipe.log.swallowed(event);
        pipe.focusing = !matches!(event.kind, MouseKind::Release(_));
        return;
    }
    if divider_drag(p, pipe, event) {
        return;
    }
    if sidebar_wheel(p, event) {
        return;
    }
    let view = p.focus.view();
    if matches!(event.kind, MouseKind::Press(_))
        && view.local(event.col, event.row).is_none()
        && let Some(other) = p.focus.pane_at(event.col, event.row)
    {
        pipe.log.focused(event, &view, other);
        let _ = p.link.send(&ClientMsg::Focus(other));
        if matches!(event.kind, MouseKind::Press(MouseButton::Right)) {
            // The menu acts on the focused pane: the one just clicked.
            send_mouse(
                p,
                MouseAction::Menu {
                    col: event.col,
                    row: event.row,
                },
            );
        } else {
            pipe.focusing = true;
        }
        return;
    }
    let action = pipe.router.route(event, &view, Instant::now());
    pipe.log.routed(event, &view, &action);
    send_mouse(p, action);
}

/// The wheel over the sidebar scrolls it. Returns whether the event was
/// one.
fn sidebar_wheel(p: &InputParams, event: &MouseEvent) -> bool {
    let MouseKind::Wheel(dir) = event.kind else {
        return false;
    };
    if !p.focus.in_sidebar(event.col) {
        return false;
    }
    let lines = match dir {
        WheelDirection::Up => -WHEEL_LINES,
        WheelDirection::Down => WHEEL_LINES,
        WheelDirection::Left | WheelDirection::Right => return true,
    };
    let _ = p.events.send(ClientEvent::SidebarScroll(lines));
    true
}

/// A plain left press on a divider takes hold of it, and every event until
/// the release moves it instead of reaching a pane. Returns whether the
/// event was about a divider.
fn divider_drag(p: &InputParams, pipe: &mut Pipeline, event: &MouseEvent) -> bool {
    if let Some(grab) = pipe.divider {
        pipe.log.swallowed(event);
        drag_divider(p, grab, event);
        if matches!(event.kind, MouseKind::Release(_)) {
            pipe.divider = None;
        }
        return true;
    }
    if matches!(event.kind, MouseKind::Press(MouseButton::Left))
        && event.mods & MOD_BITS == 0
        && let Some(grab) = p.focus.divider_at(event.col, event.row)
    {
        pipe.log.swallowed(event);
        pipe.divider = Some(grab);
        return true;
    }
    false
}

/// A drag with a divider held: a pane divider goes to the server, the
/// sidebar's edge to the main loop.
fn drag_divider(p: &InputParams, grab: DividerGrab, event: &MouseEvent) {
    if !matches!(event.kind, MouseKind::Drag(MouseButton::Left)) {
        return;
    }
    let to = grab.target(p.focus.sidebar(), event.col, event.row);
    match grab {
        DividerGrab::Pane { pane, dir } => {
            let _ = p.link.send(&ClientMsg::DragDivider { pane, dir, to });
        }
        DividerGrab::Sidebar => {
            let _ = p.events.send(ClientEvent::SidebarWidth(to));
        }
    }
}

fn send_mouse(p: &InputParams, action: MouseAction) {
    let Some(pane) = p.focus.pane() else {
        return;
    };
    let msg = match action {
        MouseAction::Forward(bytes) => ClientMsg::Input { pane, bytes },
        MouseAction::Scroll(lines) => ClientMsg::Scroll { pane, lines },
        MouseAction::Select(gesture) => {
            let _ = p.events.send(ClientEvent::Select(gesture));
            return;
        }
        MouseAction::Menu { col, row } => {
            // Like a pop-up key: what follows goes to the menu.
            p.focus.set_overlay(true);
            let _ = p.events.send(ClientEvent::Menu { col, row });
            return;
        }
        MouseAction::Ignore => return,
    };
    let _ = p.link.send(&msg);
}
