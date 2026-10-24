//! Unit and property tests for [`HostInputFilter`].

use proptest::prelude::*;

use super::*;

const NONE: ModeFlags = ModeFlags::empty();
const ALL: ModeFlags = ModeFlags::BRACKETED_PASTE.union(ModeFlags::FOCUS_IN_OUT);

fn run(chunks: &[&[u8]], modes: ModeFlags) -> (Vec<u8>, Option<HostFocus>) {
    let mut filter = HostInputFilter::new();
    let mut out = Vec::new();
    let mut focus = None;
    for chunk in chunks {
        focus = filter.filter(chunk, modes, &mut out).or(focus);
    }
    (out, focus)
}

#[test]
fn keys_pass_through_untouched() {
    let keys = b"ls\r\x1b[A\x1bOA\x1b[13;2u\x1b[15~\x1bx\x1b[\x1b";
    assert_eq!(run(&[keys], NONE).0, keys);
}

#[test]
fn paste_markers_follow_the_pane_mode() {
    let paste = b"\x1b[200~hi\x1b[201~";
    assert_eq!(run(&[paste], NONE).0, b"hi");
    assert_eq!(run(&[paste], ALL).0, paste);
}

#[test]
fn focus_events_follow_the_pane_mode_and_are_reported() {
    assert_eq!(
        run(&[b"a\x1b[Ib"], NONE),
        (b"ab".to_vec(), Some(HostFocus::Gained))
    );
    assert_eq!(
        run(&[b"\x1b[I\x1b[O"], ALL),
        (b"\x1b[I\x1b[O".to_vec(), Some(HostFocus::Lost))
    );
}

#[test]
fn focus_sequences_inside_a_paste_are_data() {
    let paste = b"\x1b[200~\x1b[I\x1b[201~";
    assert_eq!(run(&[paste], NONE), (b"\x1b[I".to_vec(), None));
}

#[test]
fn a_marker_split_across_reads_is_still_removed() {
    assert_eq!(run(&[b"x\x1b[20", b"1~y"], NONE).0, b"xy");
}

#[test]
fn lone_escape_is_never_held() {
    let mut filter = HostInputFilter::new();
    let mut out = Vec::new();
    filter.filter(b"\x1b", NONE, &mut out);
    assert_eq!(out, b"\x1b");
    filter.filter(b"\x1b[", NONE, &mut out);
    assert_eq!(out, b"\x1b\x1b[");
}

#[test]
fn a_held_prefix_that_turns_out_to_be_a_key_is_released() {
    assert_eq!(run(&[b"\x1b[2", b"~"], NONE).0, b"\x1b[2~");
}

proptest! {
    /// With every mode on, the filter is the identity, however reads split.
    #[test]
    fn identity_when_the_pane_wants_everything(
        bytes in proptest::collection::vec(prop_oneof![
            Just(0x1bu8), Just(b'['), Just(b'2'), Just(b'0'), Just(b'1'), Just(b'~'),
            Just(b'I'), Just(b'O'), any::<u8>()
        ], 0..64),
        cut in 0usize..64,
    ) {
        let cut = cut.min(bytes.len());
        let (out, _) = run(&[&bytes[..cut], &bytes[cut..]], ALL);
        let held_tail = out.len() < bytes.len();
        // Only a trailing partial marker may still be held.
        prop_assert!(bytes.starts_with(&out));
        if held_tail {
            prop_assert!(is_partial_marker(&bytes[out.len()..]));
        }
    }
}
