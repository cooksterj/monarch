//! Tests for [`PresentationHandler`], through the engine that feeds it.

use monarch_proto::ScreenSize;

use crate::{AlacrittyEngine, Engine};

/// The row-0 cells' symbols and widths, and the cursor column, after `bytes`.
fn feed(cols: u16, bytes: &str) -> (Vec<(String, u8)>, u16) {
    let mut engine = AlacrittyEngine::new(ScreenSize::new(cols, 2), 0);
    engine.feed(bytes.as_bytes(), &mut Vec::new());
    let screen = engine.screen();
    let row = screen.rows[0].iter().map(|c| (c.text(), c.width)).collect();
    (row, screen.cursor.col)
}

#[test]
fn an_emoji_with_the_selector_takes_two_cells() {
    let (row, col) = feed(10, "\u{26A0}\u{FE0F}X");
    assert_eq!(row[0], ("\u{26A0}\u{FE0F}".into(), 2));
    assert_eq!(row[1].1, 0, "the spacer: {row:?}");
    assert_eq!(row[2].0, "X", "X where the program meant it: {row:?}");
    assert_eq!(col, 3);
}

#[test]
fn without_the_selector_the_glyph_stays_narrow() {
    let (row, _) = feed(10, "\u{26A0}X");
    assert_eq!(row[0], ("\u{26A0}".into(), 1));
    assert_eq!(row[1].0, "X");
}

#[test]
fn an_already_wide_emoji_is_not_widened_again() {
    let (row, col) = feed(10, "\u{1F600}\u{FE0F}X");
    assert_eq!(row[0].1, 2);
    assert_eq!(row[2].0, "X", "{row:?}");
    assert_eq!(col, 3);
}

#[test]
fn a_second_selector_changes_nothing() {
    let (row, col) = feed(10, "\u{2764}\u{FE0F}\u{FE0F}X");
    assert_eq!(row[0].1, 2);
    assert_eq!(row[2].0, "X", "{row:?}");
    assert_eq!(col, 3);
}

#[test]
fn widening_into_the_last_column_waits_to_wrap() {
    let (row, col) = feed(4, "ab\u{2714}\u{FE0F}");
    assert_eq!(row[2], ("\u{2714}\u{FE0F}".into(), 2));
    let (_, native) = feed(4, "ab\u{4E2D}");
    assert_eq!(col, native, "the cursor as after a wide character there");
    let (row, _) = feed(4, "ab\u{2714}\u{FE0F}Z");
    assert_eq!(row[2].1, 2, "{row:?}");
}

#[test]
fn a_glyph_in_the_last_column_stays_narrow() {
    let (row, _) = feed(4, "abc\u{2714}\u{FE0F}");
    assert_eq!(row[3], ("\u{2714}\u{FE0F}".into(), 1));
}

#[test]
fn widening_inside_a_synchronized_update() {
    let (row, _) = feed(10, "\x1b[?2026h\u{26A0}\u{FE0F}X\x1b[?2026l");
    assert_eq!(row[0].1, 2);
    assert_eq!(row[2].0, "X", "{row:?}");
}

#[test]
fn the_selector_in_a_title_widens_nothing() {
    let (row, col) = feed(10, "a\x1b]2;\u{2764}\u{FE0F}\x07b");
    assert_eq!(row[0], ("a".into(), 1));
    assert_eq!(row[1].0, "b");
    assert_eq!(col, 2);
}

#[test]
fn a_cursor_move_after_the_emoji_lands_where_the_program_meant() {
    // Ink-style: the emoji, then an absolute move to column 4 (1-based).
    let (row, _) = feed(10, "\u{26A0}\u{FE0F} hi\x1b[1;4HX");
    assert_eq!(row[2].0, " ");
    assert_eq!(row[3].0, "X", "{row:?}");
    assert_eq!(row[4].0, "i");
}
