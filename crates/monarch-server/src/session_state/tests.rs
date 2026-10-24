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
