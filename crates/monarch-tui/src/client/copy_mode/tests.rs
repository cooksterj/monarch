//! Tests for [`CopyMode`].

use monarch_core::PaneId;
use monarch_proto::{Cell, GridPoint, Screen, ScreenSize, SelectShape};

use super::*;
use crate::OverlayKey::{self, Char, Ctrl, Enter, Esc};

/// A 20×4 screen showing `lines` with `history` lines above it, scrolled
/// back `offset` lines, the cursor at the bottom left.
fn screen(lines: &[&str], history: u32, offset: u32) -> Screen {
    let mut s = Screen::blank(ScreenSize::new(20, 4));
    for (r, line) in lines.iter().enumerate() {
        for (c, ch) in line.chars().enumerate() {
            s.rows[r][c] = Cell::new(ch, 1);
        }
    }
    s.history = history;
    s.display_offset = offset;
    s.cursor.row = 3;
    s
}

fn at(line: i64, col: u16) -> GridPoint {
    GridPoint { line, col }
}

fn keys(copy: &mut CopyMode, s: &Screen, keys: &[OverlayKey]) -> CopyStep {
    keys.iter().fold(CopyStep::Stay, |_, k| copy.key(*k, s))
}

#[test]
fn starts_at_the_pane_cursor_and_moves_vi_style() {
    let s = screen(&["one two-three", "", "", "$ "], 10, 0);
    let mut copy = CopyMode::new(PaneId(1), &s);
    assert_eq!(copy.cursor_cell(&s), Some((3, 0)));
    keys(&mut copy, &s, &[Char('k'), Char('k'), Char('k'), Char('l')]);
    assert_eq!(copy.cursor, at(10, 1));
    keys(&mut copy, &s, &[Char('w')]);
    assert_eq!(copy.cursor.col, 4, "next word");
    keys(&mut copy, &s, &[Char('w')]);
    assert_eq!(copy.cursor.col, 7, "punctuation is its own word");
    keys(&mut copy, &s, &[Char('e')]);
    assert_eq!(copy.cursor.col, 12, "end of `three`");
    keys(&mut copy, &s, &[Char('b'), Char('b')]);
    assert_eq!(copy.cursor.col, 7);
    keys(&mut copy, &s, &[Char('$')]);
    assert_eq!(copy.cursor.col, 12);
    keys(&mut copy, &s, &[Char('0'), Char('h')]);
    assert_eq!(copy.cursor.col, 0, "stops at the edge");
}

#[test]
fn leaving_the_view_asks_for_a_scroll_once() {
    let s = screen(&[], 10, 0);
    let mut copy = CopyMode::new(PaneId(1), &s);
    keys(&mut copy, &s, &[Char('H')]);
    assert_eq!(copy.take_scroll(), None, "still in view");
    keys(&mut copy, &s, &[Char('k')]);
    assert_eq!(copy.take_scroll(), Some(9));
    assert_eq!(copy.take_scroll(), None);
    keys(&mut copy, &s, &[Char('k')]);
    assert_eq!(
        copy.take_scroll(),
        Some(8),
        "counts from the scroll on its way, not the stale screen"
    );
    keys(&mut copy, &s, &[Char('g')]);
    assert_eq!((copy.cursor, copy.take_scroll()), (at(0, 0), Some(0)));
    keys(&mut copy, &s, &[Char('G')]);
    assert_eq!((copy.cursor, copy.take_scroll()), (at(13, 0), Some(10)));
    keys(&mut copy, &s, &[Char('j')]);
    assert_eq!(copy.cursor, at(13, 0), "no further than the last line");
}

#[test]
fn selections_toggle_change_shape_and_copy() {
    let s = screen(&[], 0, 0);
    let mut copy = CopyMode::new(PaneId(1), &s);
    assert_eq!(
        keys(&mut copy, &s, &[Char('y')]),
        CopyStep::Stay,
        "nothing selected"
    );
    keys(&mut copy, &s, &[Char('v'), Char('k'), Char('l')]);
    assert_eq!(
        copy.selection(),
        Some((at(3, 0), at(2, 1), SelectShape::Chars))
    );
    keys(&mut copy, &s, &[Ctrl('v')]);
    assert!(copy.status(&s).ends_with("block"));
    assert_eq!(
        keys(&mut copy, &s, &[Char('y')]),
        CopyStep::Copy {
            start: at(3, 0),
            end: at(2, 1),
            shape: SelectShape::Block
        }
    );
    keys(&mut copy, &s, &[Ctrl('v')]);
    assert_eq!(copy.selection(), None, "the same shape again drops it");
    keys(&mut copy, &s, &[Char('V')]);
    assert_eq!(
        keys(&mut copy, &s, &[Esc]),
        CopyStep::Stay,
        "Esc drops the selection"
    );
    assert_eq!(keys(&mut copy, &s, &[Esc]), CopyStep::Exit, "then leaves");
    assert_eq!(keys(&mut copy, &s, &[Enter]), CopyStep::Exit);
    assert_eq!(keys(&mut copy, &s, &[Char('q')]), CopyStep::Exit);
}

#[test]
fn search_prompts_repeat_and_jump_to_results() {
    let s = screen(&["a Foo b foo", "", "", ""], 5, 0);
    let mut copy = CopyMode::new(PaneId(1), &s);
    assert_eq!(
        keys(&mut copy, &s, &[Char('n')]),
        CopyStep::Stay,
        "nothing searched yet"
    );
    keys(&mut copy, &s, &[Char('/'), Char('f'), Char('x')]);
    assert_eq!(copy.status(&s), "/fx_");
    keys(&mut copy, &s, &[OverlayKey::Backspace]);
    let step = keys(&mut copy, &s, &[Char('o'), Char('o'), Enter]);
    assert_eq!(
        step,
        CopyStep::Search {
            query: "foo".into(),
            from: at(8, 0),
            backward: true
        }
    );
    assert_eq!(copy.matches(&s).len(), 2, "both, ignoring case");
    assert!(copy.found(Some((at(5, 2), at(5, 4))), &s));
    assert_eq!(copy.cursor, at(5, 2));
    assert!(!copy.found(None, &s));
    let again = keys(&mut copy, &s, &[Char('N')]);
    assert!(matches!(
        again,
        CopyStep::Search {
            backward: false,
            ..
        }
    ));
}

#[test]
fn capitals_make_a_search_exact_and_esc_leaves_the_prompt() {
    let s = screen(&["a Foo b foo", "", "", ""], 5, 0);
    let mut copy = CopyMode::new(PaneId(1), &s);
    keys(
        &mut copy,
        &s,
        &[Char('?'), Char('F'), Char('o'), Char('o'), Enter],
    );
    assert_eq!(copy.matches(&s)[0].start_col, 2, "a capital makes it exact");
    assert_eq!(copy.matches(&s).len(), 1);
    keys(&mut copy, &s, &[Char('/'), Esc]);
    assert!(
        copy.status(&s).starts_with("[copy]"),
        "Esc leaves the prompt"
    );
}

#[test]
fn a_scrolled_view_keeps_the_cursor_on_its_text() {
    let s = screen(&[], 10, 4);
    let copy = CopyMode::new(PaneId(1), &s);
    assert_eq!(
        copy.cursor,
        at(9, 0),
        "viewport row 3 of a view starting at line 6"
    );
    let later = screen(&[], 12, 6);
    assert_eq!(
        copy.cursor_cell(&later),
        Some((3, 0)),
        "same text, same cell"
    );
    assert_eq!(copy.status(&later), "[copy] 10/16");
}
