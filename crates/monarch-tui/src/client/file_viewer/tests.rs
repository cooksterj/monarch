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
