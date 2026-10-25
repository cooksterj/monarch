//! Behavioural tests for [`AlacrittyEngine`], driven by raw VT byte streams.

use monarch_proto::{CellAttrs, ModeFlags, Rgb, TermColor};

use super::*;
use crate::Palette;

fn engine(cols: u16, rows: u16) -> AlacrittyEngine {
    AlacrittyEngine::new(ScreenSize::new(cols, rows), 100)
}

fn feed(e: &mut AlacrittyEngine, bytes: &[u8]) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    e.feed(bytes, &mut events);
    events
}

fn replies(events: &[EngineEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::Reply(b) => Some(String::from_utf8_lossy(b).into_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn renders_text_and_sgr() {
    let mut e = engine(10, 3);
    feed(&mut e, b"hello\r\n\x1b[1;31mred\x1b[0m");
    let s = e.screen();
    assert_eq!(s.row_text(0), "hello");
    assert_eq!(s.row_text(1), "red");
    let red = &s.rows[1][0].style;
    assert_eq!(red.fg, TermColor::Indexed(1));
    assert!(red.attrs.contains(CellAttrs::BOLD));
    assert_eq!((s.cursor.row, s.cursor.col), (1, 3));
}

#[test]
fn wide_characters_take_two_cells() {
    let mut e = engine(6, 1);
    feed(&mut e, "界a".as_bytes());
    let row = &e.screen().rows[0];
    assert_eq!(
        (row[0].ch, row[0].width, row[1].width, row[2].ch),
        ('界', 2, 0, 'a')
    );
}

#[test]
fn answers_device_attributes_in_band() {
    let mut e = engine(10, 2);
    let events = feed(&mut e, b"\x1b[c");
    assert!(replies(&events).starts_with("\x1b[?"), "{events:?}");
}

#[test]
fn background_query_reports_host_colour() {
    let mut e = engine(10, 2);
    e.set_host_profile(HostProfile {
        palette: Palette {
            background: Rgb::new(0xfa, 0xfa, 0xfa),
            ..Palette::default()
        },
        ..HostProfile::default()
    });
    let reply = replies(&feed(&mut e, b"\x1b]11;?\x07"));
    assert!(reply.contains("rgb:fafa/fafa/fafa"), "{reply:?}");
}

#[test]
fn kitty_keyboard_push_is_honoured() {
    let mut e = engine(10, 2);
    feed(&mut e, b"\x1b[>1u");
    assert_eq!(e.modes().kitty_flags, 1);
    feed(&mut e, b"\x1b[<u");
    assert_eq!(e.modes().kitty_flags, 0);
}

#[test]
fn reports_mirrored_modes() {
    let mut e = engine(10, 2);
    feed(&mut e, b"\x1b[?2004h\x1b[?1000h\x1b[?1006h\x1b[?1049h");
    let flags = e.modes().flags;
    for f in [
        ModeFlags::BRACKETED_PASTE,
        ModeFlags::MOUSE_CLICK,
        ModeFlags::SGR_MOUSE,
        ModeFlags::ALT_SCREEN,
    ] {
        assert!(flags.contains(f), "missing {f:?}");
    }
}

#[test]
fn title_and_bell_events() {
    let mut e = engine(10, 2);
    let events = feed(&mut e, b"\x1b]2;build\x07\x07");
    assert!(events.contains(&EngineEvent::Title(Some("build".into()))));
    assert!(events.contains(&EngineEvent::Bell));
}

#[test]
fn damage_tracks_changed_rows_then_clears() {
    let mut e = engine(10, 4);
    let _ = e.take_damage();
    feed(&mut e, b"\x1b[3;1Hx");
    match e.take_damage() {
        Damage::Rows(rows) => assert!(rows.contains(&2), "{rows:?}"),
        Damage::Full => {}
    }
    let after = e.take_damage();
    assert!(
        matches!(after, Damage::Rows(ref r) if r.iter().all(|&row| row == 2)),
        "{after:?}"
    );
}

#[test]
fn synchronized_update_buffers_until_ended() {
    let mut e = engine(10, 2);
    feed(&mut e, b"\x1b[?2026hframe");
    assert!(e.sync_deadline().is_some());
    assert_eq!(e.screen().row_text(0), "");
    let mut events = Vec::new();
    e.end_sync(&mut events);
    assert_eq!(e.screen().row_text(0), "frame");
    assert!(e.sync_deadline().is_none());
}

#[test]
fn resize_reflows_and_scrollback_scrolls() {
    let mut e = engine(4, 2);
    feed(&mut e, b"abcdef");
    e.resize(ScreenSize::new(8, 2));
    assert_eq!(e.size(), ScreenSize::new(8, 2));
    assert_eq!(e.screen().row_text(0), "abcdef");

    let mut e = engine(4, 2);
    feed(&mut e, b"1\r\n2\r\n3\r\n4");
    assert_eq!(e.screen().history, 2);
    e.scroll(ScrollRequest::Top);
    let s = e.screen();
    assert_eq!((s.row_text(0), s.display_offset), ("1".into(), 2));
    assert!(!s.cursor.visible);
}

fn at(line: i64, col: u16) -> GridPoint {
    GridPoint { line, col }
}

#[test]
fn text_ranges_reach_into_scrollback_and_join_wrapped_lines() {
    let mut e = engine(10, 2);
    feed(&mut e, b"first\r\nabcdefghijKLM\r\nlast");
    // Lines: 0 "first", 1 "abcdefghij" + wrap, 2 "KLM", 3 "last"; two in history.
    assert_eq!(e.pane_state().history, 2);
    assert_eq!(
        e.text_range(at(0, 0), at(2, 9), SelectShape::Chars),
        "first\nabcdefghijKLM"
    );
    assert_eq!(
        e.text_range(at(2, 1), at(0, 2), SelectShape::Chars),
        "rst\nabcdefghijKL",
        "ends in either order"
    );
    assert_eq!(
        e.text_range(at(1, 1), at(2, 2), SelectShape::Block),
        "bc\nLM"
    );
    assert_eq!(
        e.text_range(at(3, 0), at(3, 0), SelectShape::Lines),
        "last\n"
    );
    assert_eq!(
        e.text_range(at(-5, 0), at(0, 99), SelectShape::Chars),
        "first"
    );
}

#[test]
fn search_finds_text_in_history_both_ways_and_wraps() {
    let mut e = engine(20, 2);
    feed(&mut e, b"one needle\r\ntwo\r\nthree Needle\r\nfour");
    let found = |e: &AlacrittyEngine, q: &str, from: GridPoint, back: bool| e.search(q, from, back);
    assert_eq!(
        found(&e, "needle", at(3, 0), true),
        Some((at(2, 6), at(2, 11))),
        "lower case matches any case, searching back"
    );
    assert_eq!(
        found(&e, "needle", at(2, 6), true),
        Some((at(0, 4), at(0, 9)))
    );
    assert_eq!(
        found(&e, "Needle", at(0, 4), false),
        Some((at(2, 6), at(2, 11))),
        "a capital makes it exact"
    );
    assert_eq!(
        found(&e, "Needle", at(2, 6), false),
        Some((at(2, 6), at(2, 11))),
        "wraps around to the only match"
    );
    assert_eq!(found(&e, "t.o", at(3, 0), true), None, "not a pattern");
    assert_eq!(found(&e, "", at(3, 0), true), None);
}

#[test]
fn scrolling_to_a_top_line_is_absolute_and_clamped() {
    let mut e = engine(10, 2);
    feed(&mut e, b"0\r\n1\r\n2\r\n3\r\n4");
    assert_eq!(e.pane_state().history, 3);
    e.scroll(ScrollRequest::TopLine(1));
    assert_eq!(e.screen().row_text(0), "1");
    e.scroll(ScrollRequest::TopLine(1));
    assert_eq!(
        e.pane_state().display_offset,
        2,
        "repeating changes nothing"
    );
    e.scroll(ScrollRequest::TopLine(-4));
    assert_eq!(e.screen().row_text(0), "0", "clamped to the oldest line");
    e.scroll(ScrollRequest::TopLine(99));
    assert_eq!(e.pane_state().display_offset, 0, "clamped to live output");
}

#[test]
fn text_is_the_live_screen_even_when_scrolled_back() {
    let mut e = engine(4, 2);
    feed(&mut e, "1\r\n2\r\n3\r\n界x".as_bytes());
    assert_eq!(e.text(), "3\n界x");
    e.scroll(ScrollRequest::Top);
    assert_eq!(e.screen().text(), "1\n2", "the viewport moved");
    assert_eq!(e.text(), "3\n界x", "detection still reads the bottom");
}
