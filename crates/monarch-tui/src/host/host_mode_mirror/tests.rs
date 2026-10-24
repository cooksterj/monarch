//! Tests for [`HostModeMirror`].

use super::*;

fn kitty_host() -> HostInfo {
    HostInfo {
        kitty_flags: Some(0),
        ..HostInfo::default()
    }
}

fn modes(flags: ModeFlags, kitty_flags: u8) -> TerminalModes {
    TerminalModes { flags, kitty_flags }
}

fn sync(mirror: &mut HostModeMirror, m: &TerminalModes, cursor: &Cursor) -> String {
    let mut out = Vec::new();
    mirror.sync(m, cursor, &mut out);
    String::from_utf8(out).expect("ascii")
}

#[test]
fn first_sync_writes_the_full_state() {
    let mut mirror = HostModeMirror::new(&kitty_host());
    let out = sync(&mut mirror, &TerminalModes::default(), &Cursor::default());
    for mode in [1, 1000, 1002, 1003, 1005, 1006, 1007] {
        assert!(out.contains(&format!("\x1b[?{mode}l")), "{mode} in {out:?}");
    }
    assert!(out.contains("\x1b>"));
    assert!(out.contains("\x1b[=0;1u"));
    assert!(out.contains("\x1b[2 q"));
}

#[test]
fn unchanged_state_writes_nothing() {
    let mut mirror = HostModeMirror::new(&kitty_host());
    let m = modes(ModeFlags::APP_CURSOR, 1);
    sync(&mut mirror, &m, &Cursor::default());
    assert_eq!(sync(&mut mirror, &m, &Cursor::default()), "");
}

#[test]
fn only_changes_are_written() {
    let mut mirror = HostModeMirror::new(&kitty_host());
    sync(&mut mirror, &TerminalModes::default(), &Cursor::default());
    let m = modes(ModeFlags::APP_CURSOR | ModeFlags::APP_KEYPAD, 0b1001);
    let cursor = Cursor {
        shape: CursorShape::Beam,
        blinking: true,
        ..Cursor::default()
    };
    assert_eq!(
        sync(&mut mirror, &m, &cursor),
        "\x1b[?1h\x1b=\x1b[=9;1u\x1b[5 q"
    );
}

#[test]
fn mouse_mode_switch_resets_before_setting() {
    let mut mirror = HostModeMirror::new(&HostInfo::default());
    let click = modes(ModeFlags::MOUSE_CLICK | ModeFlags::SGR_MOUSE, 0);
    sync(&mut mirror, &click, &Cursor::default());
    let drag = modes(ModeFlags::MOUSE_DRAG | ModeFlags::SGR_MOUSE, 0);
    assert_eq!(
        sync(&mut mirror, &drag, &Cursor::default()),
        "\x1b[?1000l\x1b[?1002h"
    );
}

#[test]
fn alternate_scroll_follows_the_alternate_screen() {
    let mut mirror = HostModeMirror::new(&HostInfo::default());
    let main = modes(ModeFlags::ALTERNATE_SCROLL, 0);
    assert!(sync(&mut mirror, &main, &Cursor::default()).contains("\x1b[?1007l"));
    let alt = modes(ModeFlags::ALTERNATE_SCROLL | ModeFlags::ALT_SCREEN, 0);
    assert_eq!(sync(&mut mirror, &alt, &Cursor::default()), "\x1b[?1007h");
}

#[test]
fn legacy_host_never_gets_kitty_sequences() {
    let mut mirror = HostModeMirror::new(&HostInfo::default());
    let mut out = Vec::new();
    mirror.enter(&mut out);
    mirror.sync(&modes(ModeFlags::empty(), 1), &Cursor::default(), &mut out);
    mirror.leave(&mut out);
    let out = String::from_utf8(out).expect("ascii");
    assert!(!out.contains('u'), "{out:?}");
}

#[test]
fn enter_and_leave_bracket_the_session() {
    let mut mirror = HostModeMirror::new(&kitty_host());
    let mut out = Vec::new();
    mirror.enter(&mut out);
    let entered = String::from_utf8(out.clone()).expect("ascii");
    assert!(entered.contains("\x1b[?2004h") && entered.contains("\x1b[>0u"));
    out.clear();
    mirror.leave(&mut out);
    let left = String::from_utf8(out).expect("ascii");
    assert!(left.contains("\x1b[?2004l") && left.ends_with("\x1b[<u\x1b[23;0t"));
}

#[test]
fn invalidate_forces_a_full_resync() {
    let mut mirror = HostModeMirror::new(&HostInfo::default());
    sync(&mut mirror, &TerminalModes::default(), &Cursor::default());
    mirror.invalidate();
    assert!(!sync(&mut mirror, &TerminalModes::default(), &Cursor::default()).is_empty());
}

#[test]
fn title_changes_are_written_once_and_sanitised() {
    let mut mirror = HostModeMirror::new(&HostInfo::default());
    let mut out = Vec::new();
    mirror.sync_title(Some("vim\x07 x"), &mut out);
    mirror.sync_title(Some("vim\x07 x"), &mut out);
    assert_eq!(out, b"\x1b]2;vim x\x1b\\");
    out.clear();
    mirror.sync_title(None, &mut out);
    assert_eq!(out, b"\x1b]2;\x1b\\");
}

fn captured(policy: MouseMode, flags: ModeFlags) -> String {
    let mut mirror = HostModeMirror::new(&HostInfo::default()).with_mouse(policy);
    sync(&mut mirror, &modes(flags, 0), &Cursor::default())
}

#[test]
fn auto_capture_always_tracks_drags_in_sgr() {
    let plain = captured(MouseMode::Auto, ModeFlags::empty());
    assert!(
        plain.contains("\x1b[?1002h") && plain.contains("\x1b[?1006h"),
        "{plain:?}"
    );
    let x10_pane = captured(
        MouseMode::Auto,
        ModeFlags::MOUSE_CLICK | ModeFlags::UTF8_MOUSE,
    );
    assert!(
        x10_pane.contains("\x1b[?1005l") && x10_pane.contains("\x1b[?1006h"),
        "host stays SGR: {x10_pane:?}"
    );
    let motion = captured(MouseMode::Auto, ModeFlags::MOUSE_MOTION);
    assert!(motion.contains("\x1b[?1003h"), "{motion:?}");
}

#[test]
fn captured_wheel_turns_host_alternate_scroll_off() {
    let alt = ModeFlags::ALT_SCREEN | ModeFlags::ALTERNATE_SCROLL;
    assert!(captured(MouseMode::Auto, alt).contains("\x1b[?1007l"));
    assert!(
        captured(MouseMode::Pane, alt).contains("\x1b[?1007h"),
        "pane mode without tracking leaves the host wheel"
    );
}

#[test]
fn pane_and_off_policies() {
    assert!(captured(MouseMode::Pane, ModeFlags::empty()).contains("\x1b[?1002l"));
    let wants = captured(MouseMode::Pane, ModeFlags::MOUSE_CLICK);
    assert!(
        wants.contains("\x1b[?1000h") && wants.contains("\x1b[?1006h"),
        "{wants:?}"
    );
    let off = captured(MouseMode::Off, ModeFlags::MOUSE_DRAG | ModeFlags::SGR_MOUSE);
    for mode in [1000, 1002, 1003, 1006] {
        assert!(!off.contains(&format!("\x1b[?{mode}h")), "{off:?}");
    }
}

#[test]
fn set_mouse_switches_capture_on_the_next_sync() {
    let mut mirror = HostModeMirror::new(&HostInfo::default()).with_mouse(MouseMode::Auto);
    let plain = modes(ModeFlags::empty(), 0);
    assert!(sync(&mut mirror, &plain, &Cursor::default()).contains("\x1b[?1002h"));
    mirror.set_mouse(MouseMode::Off);
    let off = sync(&mut mirror, &plain, &Cursor::default());
    assert!(
        off.contains("\x1b[?1002l") && off.contains("\x1b[?1006l"),
        "{off:?}"
    );
}
