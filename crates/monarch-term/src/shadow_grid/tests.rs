//! Tests for [`ShadowGrid`] driven by a real (in-memory) engine.

use super::*;
use crate::Terminal;

fn term() -> Terminal {
    Terminal::new(ScreenSize::new(10, 4), 100)
}

fn feed(t: &mut Terminal, bytes: &[u8]) {
    t.feed(bytes, &mut Vec::new());
}

fn primed(t: &mut Terminal) -> ShadowGrid {
    let mut shadow = ShadowGrid::new(t.size());
    shadow.apply(t.capture(true));
    shadow
}

#[test]
fn reports_only_rows_whose_cells_changed() {
    let mut t = term();
    let mut shadow = primed(&mut t);
    feed(&mut t, b"\x1b[3;1Hhello");
    let change = shadow.apply(t.capture(false)).expect("a change");
    assert_eq!(change.lines, [2]);
    assert_eq!(shadow.row(2).map(|r| r[0].ch), Some('h'));
}

#[test]
fn rewriting_identical_content_is_not_a_change() {
    let mut t = term();
    feed(&mut t, b"same");
    let mut shadow = primed(&mut t);
    feed(&mut t, b"\x1b[1;1Hsame");
    assert_eq!(
        shadow.apply(t.capture(false)),
        None,
        "damage over-reports, the diff does not"
    );
}

#[test]
fn cursor_movement_alone_is_a_state_only_change() {
    let mut t = term();
    let mut shadow = primed(&mut t);
    feed(&mut t, b"\x1b[2;5H");
    let change = shadow.apply(t.capture(false)).expect("a change");
    assert!(change.is_state_only());
    assert_eq!((change.state.cursor.row, change.state.cursor.col), (1, 4));
}

#[test]
fn nothing_written_means_no_change() {
    let mut t = term();
    let mut shadow = primed(&mut t);
    assert_eq!(shadow.apply(t.capture(false)), None);
}

#[test]
fn resize_starts_over_with_a_full_frame() {
    let mut t = term();
    feed(&mut t, b"abc");
    let mut shadow = primed(&mut t);
    t.resize(ScreenSize::new(20, 6));
    let change = shadow.apply(t.capture(false)).expect("a change");
    assert!(change.resized);
    assert_eq!(shadow.size(), ScreenSize::new(20, 6));
    assert_eq!(shadow.rows().len(), 6);
    assert_eq!(shadow.row(0).map(|r| r[0].ch), Some('a'));
}

#[test]
fn shadow_matches_the_engine_screen_after_scrolling_output() {
    let mut t = term();
    let mut shadow = primed(&mut t);
    for i in 0..12 {
        feed(&mut t, format!("line {i}\r\n").as_bytes());
        shadow.apply(t.capture(false));
    }
    assert_eq!(shadow.rows(), t.screen().rows.as_slice());
}

#[test]
fn scrolled_back_view_is_copied_from_history() {
    let mut t = term();
    for i in 0..12 {
        feed(&mut t, format!("line {i}\r\n").as_bytes());
    }
    t.scroll(crate::ScrollRequest::Lines(3));
    let mut shadow = ShadowGrid::new(t.size());
    shadow.apply(t.capture(true));
    assert_eq!(shadow.rows(), t.screen().rows.as_slice());
    assert_eq!(shadow.state().display_offset, 3);
}
