//! Tests for [`AttachState`].

use monarch_proto::{
    CellAttrs, CellStyle, Delta, PaneLayout, PaneState, PaneSummary, RowDelta, Run, ScreenSize,
    Snapshot, StyleDef, StyleId, TabSummary,
};

use super::*;

fn pane(id: u32, title: Option<&str>) -> PaneSummary {
    PaneSummary {
        pane: PaneId(id),
        title: title.map(Into::into),
        command: None,
        place: None,
    }
}

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
        tabs: vec![TabSummary {
            focus: PaneId(1),
            zoomed: false,
            panes: vec![pane(1, Some("sh"))],
        }],
        active_tab: 0,
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
            ServerMsg::Clipboard("copied".into()),
            ServerMsg::PaneExited { pane: PaneId(1) },
        ],
    );
    assert_eq!(state.focused_title(), Some("vim"));
    assert_eq!(out.host_write, [7]);
    assert_eq!(out.clipboard, ["copied"]);
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

#[test]
fn focus_view_reports_rect_modes_and_scrollback() {
    let mut state = AttachState::new();
    assert_eq!(state.focus_view(), FocusView::default());
    let mut scrolled = PaneState {
        display_offset: 4,
        ..PaneState::default()
    };
    scrolled.modes.flags = ModeFlags::ALT_SCREEN;
    let snap = ServerMsg::Snapshot(Snapshot {
        pane: PaneId(1),
        generation: 1,
        seq: 1,
        size: SIZE,
        rows: vec![vec![], vec![]],
        state: scrolled,
    });
    apply_all(&mut state, vec![layout(1), snap]);
    let view = state.focus_view();
    assert_eq!((view.pane, view.cols, view.rows), (Some(PaneId(1)), 10, 2));
    assert!(view.scrolled);
    assert!(view.modes.contains(ModeFlags::ALT_SCREEN));
}

#[test]
fn tab_labels_follow_titles_zoom_and_length() {
    let mut state = AttachState::new();
    assert_eq!(
        state.tab_labels(),
        (Vec::new(), 0),
        "nothing before a layout"
    );
    let ServerMsg::Layout(mut msg) = layout(1) else {
        unreachable!("layout() builds a layout");
    };
    msg.tabs.push(TabSummary {
        focus: PaneId(2),
        zoomed: true,
        panes: vec![pane(2, Some("a very long title that will not fit"))],
    });
    msg.tabs.push(TabSummary {
        focus: PaneId(3),
        zoomed: false,
        panes: vec![pane(3, None)],
    });
    msg.active_tab = 1;
    apply_all(&mut state, vec![ServerMsg::Layout(msg)]);
    let (labels, active) = state.tab_labels();
    assert_eq!(labels, ["1 sh", "2 a very long title t… [Z]", "3"]);
    assert_eq!(active, 1);
    apply_all(
        &mut state,
        vec![ServerMsg::Title {
            pane: PaneId(2),
            title: Some("htop".into()),
        }],
    );
    assert_eq!(
        state.tab_labels().0[1],
        "2 htop [Z]",
        "hidden tabs follow titles"
    );
}

#[test]
fn panes_without_a_title_are_named_after_their_program() {
    let mut state = AttachState::new();
    let ServerMsg::Layout(mut msg) = layout(1) else {
        unreachable!("layout() builds a layout");
    };
    msg.tabs.push(TabSummary {
        focus: PaneId(2),
        zoomed: false,
        panes: vec![pane(2, Some("htop"))],
    });
    msg.tabs.push(TabSummary {
        focus: PaneId(3),
        zoomed: false,
        panes: vec![pane(3, Some(" "))],
    });
    apply_all(
        &mut state,
        vec![
            ServerMsg::Layout(msg),
            ServerMsg::Command {
                pane: PaneId(3),
                command: Some("zsh".into()),
            },
            ServerMsg::Command {
                pane: PaneId(2),
                command: Some("vim".into()),
            },
        ],
    );
    let labels = state.tab_labels().0;
    assert_eq!(labels[2], "3 zsh", "no title: named after the program");
    assert_eq!(labels[1], "2 htop", "a title wins");
}

#[test]
fn all_panes_lists_every_tab_with_live_titles() {
    let mut state = AttachState::new();
    assert!(state.all_panes().is_empty());
    let ServerMsg::Layout(mut msg) = layout(1) else {
        unreachable!("layout() builds a layout");
    };
    msg.tabs.push(TabSummary {
        focus: PaneId(3),
        zoomed: false,
        panes: vec![pane(2, Some("vim")), pane(3, None)],
    });
    apply_all(
        &mut state,
        vec![
            ServerMsg::Layout(msg),
            ServerMsg::Title {
                pane: PaneId(3),
                title: Some("htop".into()),
            },
        ],
    );
    assert_eq!(
        state.all_panes(),
        [
            (0, PaneId(1), "sh"),
            (1, PaneId(2), "vim"),
            (1, PaneId(3), "htop")
        ]
    );
}
