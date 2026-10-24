//! Tests for [`FrameEncoder`]: messages decode back to the shadow grid.

use monarch_proto::{RowCodec, ScreenSize, StyleCache};
use monarch_term::Terminal;

use monarch_core::PaneId;

use super::*;

const HEADER: FrameHeader = FrameHeader {
    pane: PaneId(1),
    generation: 3,
    seq: 9,
};

fn shadow_of(bytes: &[u8]) -> (Terminal, ShadowGrid) {
    let mut term = Terminal::new(ScreenSize::new(12, 3), 0);
    term.feed(bytes, &mut Vec::new());
    let mut shadow = ShadowGrid::new(term.size());
    shadow.apply(term.capture(true));
    (term, shadow)
}

/// Applies messages the way a client would, returning the decoded rows.
fn decode(msgs: &[ServerMsg], cache: &mut StyleCache) -> Vec<Vec<monarch_proto::Cell>> {
    let mut rows = Vec::new();
    for msg in msgs {
        match msg {
            ServerMsg::ResetStyles => cache.clear(),
            ServerMsg::DefineStyle(def) => cache.define(*def),
            ServerMsg::Snapshot(s) => {
                rows = s
                    .rows
                    .iter()
                    .map(|r| RowCodec::decode(r, cache, s.size.cols))
                    .collect();
            }
            ServerMsg::Delta(d) => {
                rows = d
                    .rows
                    .iter()
                    .map(|r| RowCodec::decode(&r.runs, cache, 12))
                    .collect();
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    rows
}

#[test]
fn snapshot_decodes_to_the_shadow_grid() {
    let (_, shadow) = shadow_of(b"\x1b[1;31mred\x1b[0m plain\r\n\x1b[44mblue bg");
    let mut encoder = FrameEncoder::new();
    let msgs = encoder.snapshot(HEADER, &shadow);
    assert!(matches!(msgs.last(), Some(ServerMsg::Snapshot(s)) if s.seq == 9 && s.generation == 3));
    assert_eq!(decode(&msgs, &mut StyleCache::new()), shadow.rows());
}

#[test]
fn styles_are_defined_once_per_connection() {
    let (mut term, mut shadow) = shadow_of(b"\x1b[1mbold");
    let mut encoder = FrameEncoder::new();
    let first = encoder.snapshot(HEADER, &shadow);
    assert!(first.iter().any(|m| matches!(m, ServerMsg::DefineStyle(_))));
    term.feed(b"\r\n\x1b[1mmore", &mut Vec::new());
    let change = shadow.apply(term.capture(false)).expect("change");
    let second = encoder.delta(HEADER, &shadow, &change.lines);
    assert!(
        !second
            .iter()
            .any(|m| matches!(m, ServerMsg::DefineStyle(_))),
        "bold already defined"
    );
}

#[test]
fn delta_carries_only_the_given_rows() {
    let (mut term, mut shadow) = shadow_of(b"one");
    term.feed(b"\x1b[3;1Hthree", &mut Vec::new());
    let change = shadow.apply(term.capture(false)).expect("change");
    let msgs = FrameEncoder::new().delta(HEADER, &shadow, &change.lines);
    let Some(ServerMsg::Delta(delta)) = msgs.last() else {
        panic!("no delta");
    };
    assert_eq!(delta.rows.iter().map(|r| r.line).collect::<Vec<_>>(), [2]);
}

#[test]
fn reset_makes_the_next_message_start_over() {
    let (_, shadow) = shadow_of(b"\x1b[1mbold");
    let mut encoder = FrameEncoder::new();
    let _ = encoder.snapshot(HEADER, &shadow);
    encoder.reset();
    let msgs = encoder.snapshot(HEADER, &shadow);
    assert_eq!(msgs.first(), Some(&ServerMsg::ResetStyles));
    assert!(
        msgs.iter().any(|m| matches!(m, ServerMsg::DefineStyle(_))),
        "styles redefined"
    );
    let mut stale_cache = StyleCache::new();
    assert_eq!(decode(&msgs, &mut stale_cache), shadow.rows());
}
