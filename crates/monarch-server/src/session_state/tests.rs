//! Tests for [`SessionState`] with in-memory panes (no PTYs).

use monarch_proto::{ServerMsg, Snapshot};
use tokio::sync::mpsc::{self, Receiver};

use super::*;
use crate::test_support::FakePane;

fn fake() -> Box<dyn PaneIo> {
    FakePane::boxed(DEFAULT_SIZE)
}

fn session_with_panes(n: usize) -> SessionState {
    let mut s = SessionState::default();
    for _ in 0..n {
        let id = s.allocate_pane_id();
        s.insert_pane(id, fake());
    }
    s
}

fn write(s: &SessionState, pane: PaneId, bytes: &[u8]) {
    let io = s.pane(pane).expect("pane");
    io.shared().lock().feed(bytes, &mut Vec::new());
    io.shared().mark_dirty();
}

fn drain(rx: &mut Receiver<Vec<ServerMsg>>) -> Vec<ServerMsg> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .flatten()
        .collect()
}

fn snapshot(msgs: &[ServerMsg]) -> Option<&Snapshot> {
    msgs.iter().find_map(|m| match m {
        ServerMsg::Snapshot(s) => Some(s),
        _ => None,
    })
}

#[test]
fn new_client_gets_layout_then_a_snapshot_of_the_focused_pane() {
    let mut s = session_with_panes(2);
    let (tx, mut rx) = mpsc::channel(8);
    s.add_client(tx);
    s.render_frame();
    let msgs = drain(&mut rx);
    assert!(matches!(&msgs[0], ServerMsg::Layout(l) if l.focus == Some(PaneId(1))));
    assert_eq!(snapshot(&msgs).map(|s| s.pane), Some(PaneId(1)));
}

#[test]
fn output_becomes_deltas_and_idle_frames_send_nothing() {
    let mut s = session_with_panes(1);
    let (tx, mut rx) = mpsc::channel(8);
    s.add_client(tx);
    s.render_frame();
    let first = snapshot(&drain(&mut rx)).expect("snapshot").seq;
    write(&s, PaneId(1), b"hello");
    s.render_frame();
    let msgs = drain(&mut rx);
    let Some(ServerMsg::Delta(delta)) = msgs.last() else {
        panic!("expected a delta, got {msgs:?}");
    };
    assert!(delta.follows(delta.generation, first));
    assert_eq!(delta.rows.len(), 1);
    s.render_frame();
    assert!(drain(&mut rx).is_empty(), "nothing changed, nothing sent");
}

#[test]
fn hidden_panes_cost_nothing_until_focused() {
    let mut s = session_with_panes(2);
    let (tx, mut rx) = mpsc::channel(8);
    s.add_client(tx);
    s.render_frame();
    drain(&mut rx);
    write(&s, PaneId(2), b"background output");
    s.render_frame();
    assert!(
        drain(&mut rx).is_empty(),
        "hidden pane output is not rendered"
    );
    assert!(s.set_focus(PaneId(2)));
    s.render_frame();
    let msgs = drain(&mut rx);
    let snap = snapshot(&msgs).expect("snapshot on reveal");
    assert_eq!(snap.pane, PaneId(2));
    assert!(
        snap.rows[0]
            .iter()
            .any(|run| run.text.contains("background"))
    );
}

#[test]
fn a_stalled_client_is_resynced_without_affecting_others() {
    let mut s = session_with_panes(1);
    let (slow_tx, mut slow_rx) = mpsc::channel(1);
    let (fast_tx, mut fast_rx) = mpsc::channel(64);
    s.add_client(slow_tx);
    s.add_client(fast_tx);
    for i in 0..5 {
        write(&s, PaneId(1), format!("line {i}\r\n").as_bytes());
        s.render_frame();
    }
    assert_eq!(
        drain(&mut fast_rx)
            .iter()
            .filter(|m| matches!(m, ServerMsg::Delta(_)))
            .count(),
        4
    );
    drain(&mut slow_rx);
    s.render_frame();
    let msgs = drain(&mut slow_rx);
    assert!(msgs.contains(&ServerMsg::ResetStyles) || snapshot(&msgs).is_some());
    let snap = snapshot(&msgs).expect("resync snapshot");
    assert!(snap.rows[4].iter().any(|run| run.text.contains("line 4")));
}

#[test]
fn resize_reaches_every_pane_and_resends_snapshots() {
    let mut s = session_with_panes(2);
    let (tx, mut rx) = mpsc::channel(8);
    s.add_client(tx);
    s.render_frame();
    drain(&mut rx);
    let size = ScreenSize::new(100, 30);
    s.resize(size);
    for id in s.pane_ids() {
        assert_eq!(s.pane(id).expect("pane").shared().lock().size(), size);
    }
    s.render_frame();
    let msgs = drain(&mut rx);
    assert!(matches!(&msgs[0], ServerMsg::Layout(l) if l.size == size));
    assert_eq!(snapshot(&msgs).map(|s| s.size), Some(size));
}

#[test]
fn removing_the_focused_pane_moves_focus_and_reports_emptiness() {
    let mut s = session_with_panes(2);
    assert!(!s.remove_pane(PaneId(1)));
    assert_eq!(s.focus(), Some(PaneId(2)));
    assert!(s.remove_pane(PaneId(2)));
    assert_eq!(s.focus(), None);
}

#[test]
fn closed_clients_are_dropped() {
    let mut s = session_with_panes(1);
    let (tx, rx) = mpsc::channel(8);
    s.add_client(tx);
    drop(rx);
    s.render_frame();
    assert_eq!(s.client_count(), 0);
}

mod tabs_and_splits {
    use std::sync::atomic::Ordering;

    use monarch_core::{Action, Direction, SplitAxis};

    use super::*;
    use crate::{ActionEffect, Placement};

    const A: PaneId = PaneId(1);
    const B: PaneId = PaneId(2);

    /// One pane split side by side: A | B, B focused.
    fn split() -> SessionState {
        let mut s = session_with_panes(1);
        let effect = s.apply_action(Action::Split(SplitAxis::Columns));
        assert_eq!(
            effect,
            ActionEffect::Spawn(Placement::Split(SplitAxis::Columns))
        );
        let id = s.allocate_pane_id();
        s.insert_pane_at(id, fake(), Placement::Split(SplitAxis::Columns));
        s
    }

    fn size_of(s: &SessionState, pane: PaneId) -> ScreenSize {
        s.pane(pane).expect("pane").shared().lock().size()
    }

    #[test]
    fn a_split_shows_both_panes_sized_to_their_halves() {
        let s = split();
        let layout = s.layout();
        assert_eq!(layout.focus, Some(B));
        let places: Vec<_> = layout.panes.iter().map(|p| (p.pane, p.x, p.size)).collect();
        assert_eq!(
            places,
            [
                (A, 0, ScreenSize::new(39, 24)),
                (B, 40, ScreenSize::new(40, 24))
            ]
        );
        assert_eq!(size_of(&s, A), ScreenSize::new(39, 24), "the PTY follows");
    }

    #[test]
    fn focus_moves_and_zoom_fills_the_window() {
        let mut s = split();
        s.apply_action(Action::Focus(Direction::Left));
        assert_eq!(s.focus(), Some(A));
        s.apply_action(Action::ToggleZoom);
        assert_eq!(s.visible_panes(), [A]);
        assert_eq!(size_of(&s, A), DEFAULT_SIZE);
        assert_eq!(
            size_of(&s, B),
            ScreenSize::new(40, 24),
            "B keeps its split size"
        );
        s.apply_action(Action::ToggleZoom);
        assert_eq!(s.visible_panes(), [A, B]);
    }

    #[test]
    fn resize_moves_the_divider() {
        let mut s = split();
        s.apply_action(Action::Resize(Direction::Left));
        assert_eq!(size_of(&s, A), ScreenSize::new(37, 24));
    }

    #[test]
    fn switching_tabs_snapshots_the_panes_that_appear() {
        let mut s = session_with_panes(1);
        let (tx, mut rx) = mpsc::channel(8);
        s.add_client(tx);
        s.render_frame();
        drain(&mut rx);
        s.apply_action(Action::NewTab);
        s.insert_pane_at(B, fake(), Placement::Tab { activate: true });
        assert_eq!((s.active_tab(), s.visible_panes()), (1, vec![B]));
        s.render_frame();
        assert_eq!(snapshot(&drain(&mut rx)).map(|s| s.pane), Some(B));
        s.apply_action(Action::SelectTab(1));
        s.render_frame();
        let msgs = drain(&mut rx);
        assert!(matches!(&msgs[0], ServerMsg::Layout(l) if l.focus == Some(A)));
        assert_eq!(
            snapshot(&msgs).map(|s| s.pane),
            Some(A),
            "A may have changed while hidden"
        );
        s.apply_action(Action::SelectTab(9));
        assert_eq!(s.active_tab(), 0, "no tab 9");
    }

    #[test]
    fn close_hangs_up_and_the_exit_hands_focus_on() {
        let mut s = session_with_panes(1);
        let (io, closed) = FakePane::tracked();
        s.insert_pane_at(B, io, Placement::Split(SplitAxis::Rows));
        s.apply_action(Action::ClosePane);
        assert!(closed.load(Ordering::SeqCst));
        assert_eq!(
            s.visible_panes(),
            [A, B],
            "the pane stays until its program exits"
        );
        assert!(!s.remove_pane(B));
        assert_eq!((s.focus(), s.visible_panes()), (Some(A), vec![A]));
        assert_eq!(size_of(&s, A), DEFAULT_SIZE, "A takes the space back");
    }

    #[test]
    fn focusing_a_pane_in_another_tab_shows_that_tab() {
        let mut s = split();
        let c = s.allocate_pane_id();
        s.insert_pane_at(c, fake(), Placement::Tab { activate: true });
        s.set_title(A, Some("vim".into()));
        let tabs = s.layout().tabs;
        let names = |i: usize| -> Vec<_> {
            tabs[i]
                .panes
                .iter()
                .map(|p| (p.pane, p.title.clone()))
                .collect()
        };
        assert_eq!(names(0), [(A, Some("vim".into())), (B, None)]);
        assert_eq!(names(1), [(c, None)]);
        s.apply_action(Action::FocusPane(A));
        assert_eq!((s.active_tab(), s.focus()), (0, Some(A)));
        s.apply_action(Action::FocusPane(PaneId(99)));
        assert_eq!(s.focus(), Some(A), "unknown panes are ignored");
    }

    #[test]
    fn closing_a_tab_shows_a_neighbour() {
        let mut s = session_with_panes(1);
        s.insert_pane_at(B, fake(), Placement::Tab { activate: true });
        assert!(!s.remove_pane(B));
        assert_eq!((s.active_tab(), s.tabs().len(), s.focus()), (0, 1, Some(A)));
        assert!(s.remove_pane(A), "the session is empty");
        assert_eq!(s.apply_action(Action::FocusNext), ActionEffect::Done);
    }
}
