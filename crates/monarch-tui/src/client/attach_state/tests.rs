//! Tests for [`AttachState`].

use monarch_proto::{
    CellAttrs, CellStyle, Delta, PaneLayout, PaneState, RowDelta, Run, ScreenSize, Snapshot,
    StyleDef, StyleId,
};

use super::*;

const SIZE: ScreenSize = ScreenSize { cols: 10, rows: 2 };

fn layout(generation: u32) -> ServerMsg {
    ServerMsg::Layout(LayoutMsg {
        generation,
        size: SIZE,
        focus: Some(PaneId(1)),
        panes: vec![PaneLayout {
            pane: PaneId(1),
            x: 0,
            y: 0,
            size: SIZE,
            title: Some("sh".into()),
        }],
    })
}

fn run(style: u32, text: &str) -> Run {
    Run {
        style: StyleId(style),
        width: 1,
        cells: u16::try_from(text.len()).expect("short"),
        text: text.into(),
    }
}

fn snapshot(seq: u64) -> ServerMsg {
    ServerMsg::Snapshot(Snapshot {
        pane: PaneId(1),
        generation: 1,
        seq,
        size: SIZE,
        rows: vec![vec![run(1, "bold")], vec![]],
        state: PaneState::default(),
    })
}

fn delta(seq: u64, text: &str) -> ServerMsg {
    ServerMsg::Delta(Delta {
        pane: PaneId(1),
        generation: 1,
        seq,
        rows: vec![RowDelta {
            line: 1,
            runs: vec![run(0, text)],
        }],
        state: PaneState::default(),
    })
}

fn apply_all(state: &mut AttachState, msgs: Vec<ServerMsg>) -> Applied {
    let mut out = Applied::default();
    msgs.into_iter().for_each(|m| state.apply(m, &mut out));
    out
}

fn bold() -> ServerMsg {
    ServerMsg::DefineStyle(StyleDef {
        id: StyleId(1),
        style: CellStyle {
            attrs: CellAttrs::BOLD,
            ..CellStyle::default()
        },
    })
}

#[test]
fn layout_styles_snapshot_and_delta_build_the_screen() {
    let mut state = AttachState::new();
    let out = apply_all(
        &mut state,
        vec![layout(1), bold(), snapshot(3), delta(4, "next")],
    );
    assert!(out.redraw && out.resync.is_empty());
    let screen = state.screen(PaneId(1)).expect("pane");
    assert_eq!(screen.text(), "bold\nnext");
    assert!(screen.rows[0][0].style.attrs.contains(CellAttrs::BOLD));
    assert_eq!(state.focused_title(), Some("sh"));
}

#[test]
fn a_gap_asks_for_one_resync_until_a_snapshot_arrives() {
    let mut state = AttachState::new();
    apply_all(&mut state, vec![layout(1), snapshot(3)]);
    let out = apply_all(&mut state, vec![delta(5, "lost"), delta(6, "lost")]);
    assert_eq!(out.resync, [PaneId(1)], "asked once");
    apply_all(&mut state, vec![snapshot(6)]);
    let out = apply_all(&mut state, vec![delta(8, "gap")]);
    assert_eq!(out.resync, [PaneId(1)], "asks again after recovering");
}

#[test]
fn deltas_before_any_snapshot_trigger_a_resync() {
    let mut state = AttachState::new();
    let out = apply_all(&mut state, vec![delta(1, "x")]);
    assert_eq!(out.resync, [PaneId(1)]);
}

#[test]
fn titles_host_writes_and_exits_are_reported() {
    let mut state = AttachState::new();
    apply_all(&mut state, vec![layout(1), snapshot(1)]);
    let out = apply_all(
        &mut state,
        vec![
            ServerMsg::Title {
                pane: PaneId(1),
                title: Some("vim".into()),
            },
            ServerMsg::HostWrite(vec![7]),
            ServerMsg::PaneExited { pane: PaneId(1) },
        ],
    );
    assert_eq!(state.focused_title(), Some("vim"));
    assert_eq!(out.host_write, [7]);
    assert_eq!(out.exited, [PaneId(1)]);
    assert!(state.screen(PaneId(1)).is_none());
}

#[test]
fn reset_styles_forgets_definitions() {
    let mut state = AttachState::new();
    apply_all(
        &mut state,
        vec![layout(1), bold(), ServerMsg::ResetStyles, snapshot(1)],
    );
    let screen = state.screen(PaneId(1)).expect("pane");
    assert!(screen.rows[0][0].style.is_plain());
}
