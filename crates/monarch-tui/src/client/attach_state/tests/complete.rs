//! Tests for [`AttachState::complete`].

use super::*;

#[test]
fn complete_once_every_laid_out_pane_has_a_screen() {
    let mut state = AttachState::new();
    assert!(!state.complete(), "no layout yet");
    apply_all(&mut state, vec![layout(1)]);
    assert!(!state.complete(), "the pane's snapshot has not arrived");
    apply_all(&mut state, vec![snapshot(1)]);
    assert!(state.complete());
}

#[test]
fn a_pane_that_left_the_layout_is_not_waited_for() {
    let mut state = AttachState::new();
    apply_all(&mut state, vec![layout(1), snapshot(1)]);
    let ServerMsg::Layout(mut empty) = layout(2) else {
        unreachable!("layout() builds a layout")
    };
    empty.panes.clear();
    apply_all(&mut state, vec![ServerMsg::Layout(empty)]);
    assert!(state.complete(), "nothing laid out is missing");
}
