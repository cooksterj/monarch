use OverlayKey::{Char, Ctrl, Esc};
use monarch_core::SyntaxToken;
use monarch_proto::{CodeLine, CodeSpan};

use super::*;

fn viewer(lines: usize) -> FileViewer {
    let mut v = FileViewer::new(PaneId(3), "src/a.rs");
    let line = |i| CodeLine {
        spans: vec![CodeSpan::new(SyntaxToken::Plain, format!("line {}", i + 1))],
    };
    v.load(Ok(CodeFile {
        path: "src/a.rs".into(),
        language: Some("rust".into()),
        lines: (0..lines).map(line).collect(),
        truncated: false,
    }));
    v.set_page(10);
    v
}

fn keys(v: &mut FileViewer, keys: &[OverlayKey]) -> FileViewerStep {
    keys.iter().fold(FileViewerStep::Open, |_, k| v.key(*k))
}

#[test]
fn moves_page_and_counts_keep_the_cursor_in_view() {
    let mut v = viewer(100);
    keys(&mut v, &[Char('j'), Char('j'), Char('k')]);
    assert_eq!((v.cursor(), v.top()), (1, 0));
    keys(&mut v, &[Char(' ')]);
    assert_eq!(
        (v.cursor(), v.top()),
        (11, 2),
        "the cursor's line is the last shown"
    );
    keys(&mut v, &[Char('4'), Char('2'), Char('G')]);
    assert_eq!(v.cursor(), 41, "42G goes to line 42");
    keys(&mut v, &[Char('G')]);
    assert_eq!((v.cursor(), v.top()), (99, 90));
    keys(&mut v, &[Ctrl('u'), Char('g')]);
    assert_eq!((v.cursor(), v.top()), (0, 0));
    keys(&mut v, &[Char('3'), Char('j'), Char('0'), Char('j')]);
    assert_eq!(v.cursor(), 4, "a lone 0 is no count");
    keys(&mut v, &[Char('k'); 9]);
    assert_eq!(v.cursor(), 0, "no further than the first line");
}

#[test]
fn selecting_then_asking_quotes_those_lines_with_their_place() {
    let mut v = viewer(20);
    keys(
        &mut v,
        &[Char('4'), Char('G'), Char('V'), Char('j'), Char('j')],
    );
    assert_eq!(v.selection(), Some((3, 5)));
    let FileViewerStep::Ask(quote) = keys(&mut v, &[Char('a')]) else {
        panic!("not an ask");
    };
    assert_eq!(quote.summary(), "src/a.rs:4-6");
    assert_eq!(
        keys(&mut v, &[Char('y')]),
        FileViewerStep::Copy("line 4\nline 5\nline 6".into())
    );
    assert_eq!(
        keys(&mut v, &[Esc]),
        FileViewerStep::Open,
        "Esc clears first"
    );
    assert_eq!(v.selection(), None);
    let FileViewerStep::Ask(one) = keys(&mut v, &[Char('a')]) else {
        panic!("not an ask");
    };
    assert_eq!(
        one.summary(),
        "src/a.rs:6",
        "no selection: the cursor's line"
    );
    assert_eq!(keys(&mut v, &[Esc]), FileViewerStep::Close);
}

#[test]
fn loading_and_failed_files_offer_nothing_to_ask_but_still_close() {
    let mut v = FileViewer::new(PaneId(1), "bin");
    assert_eq!(v.file(), None);
    assert_eq!(keys(&mut v, &[Char('j'), Char('a')]), FileViewerStep::Open);
    v.load(Err("bin is a binary file".into()));
    assert_eq!(v.quote(), None);
    assert_eq!(
        keys(&mut v, &[Char('e')]),
        FileViewerStep::Edit("bin".into())
    );
    assert_eq!((v.pane(), v.path()), (PaneId(1), "bin"));
    assert_eq!(keys(&mut v, &[Char('q')]), FileViewerStep::Close);
}

#[test]
fn jumping_shows_the_line_with_context_above_and_clamps_to_the_file() {
    let mut v = viewer(100);
    v.jump(42);
    assert_eq!(
        (v.cursor(), v.top()),
        (41, 38),
        "a third of the page above it"
    );
    v.jump(1);
    assert_eq!((v.cursor(), v.top()), (0, 0));
    v.jump(500);
    assert_eq!(v.cursor(), 99, "past the end: the last line");
    v.jump(0);
    assert_eq!(v.cursor(), 0, "0 is the first line");
}

#[test]
fn slash_searches_n_and_shift_n_go_again_and_wrap() {
    let mut v = viewer(100);
    keys(
        &mut v,
        &[
            Char('/'),
            Char('l'),
            Char('i'),
            Char('n'),
            Char('e'),
            Char(' '),
            Char('5'),
        ],
    );
    assert_eq!(v.typing(), Some("line 5"), "keys type the query");
    assert_eq!(v.cursor(), 0, "nothing moves while typing");
    keys(&mut v, &[OverlayKey::Enter]);
    assert_eq!((v.cursor(), v.query()), (4, Some("line 5")), "line 5");
    keys(&mut v, &[Char('n')]);
    assert_eq!(
        (v.cursor(), v.top()),
        (49, 46),
        "then line 50, context above"
    );
    keys(&mut v, &[Char('N'), Char('N')]);
    assert_eq!(v.cursor(), 58, "back past the top to line 59");
    assert_eq!(v.matches_in("Line 5 line 5"), [(0, 6), (7, 13)]);
    keys(&mut v, &[Char('/'), Char('z'), OverlayKey::Enter]);
    assert_eq!(v.status(), Some("no line has \"z\""));
    assert_eq!(v.cursor(), 58, "stays put");
    keys(&mut v, &[Char('/'), Char('x'), Esc]);
    assert_eq!(
        (v.typing(), v.query()),
        (None, Some("z")),
        "Esc drops the typing"
    );
}

#[test]
fn h_and_l_scroll_sideways_and_0_comes_back() {
    let mut v = viewer(3);
    keys(&mut v, &[Char('l'), Char('l'), OverlayKey::Right]);
    assert_eq!(v.left(), 24);
    keys(&mut v, &[Char('h')]);
    assert_eq!(v.left(), 16);
    keys(&mut v, &[Char('0')]);
    assert_eq!(v.left(), 0);
    keys(&mut v, &[Char('h'), OverlayKey::Left]);
    assert_eq!(v.left(), 0, "never past the start");
}
