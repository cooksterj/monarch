//! Unit and property tests for [`HostProbe`].

use monarch_proto::Rgb;
use proptest::prelude::*;

use super::*;

const KITTY_HOST: &[u8] = b"\x1b]10;rgb:dcdc/dfdf/e4e4\x1b\\\x1b]11;rgb:2828/2c2c/3434\x1b\\\
\x1b]12;rgb:ffff/ffff/ffff\x07\x1b[6;20;10t\x1b[?2026;2$y\x1b[?0u\x1b[?62;22c";

fn run(chunks: &[&[u8]]) -> (HostProbe, Vec<u8>) {
    let mut probe = HostProbe::new();
    let mut input = Vec::new();
    for chunk in chunks {
        probe.feed(chunk, &mut input);
    }
    (probe, input)
}

#[test]
fn collects_every_answer() {
    let (probe, input) = run(&[KITTY_HOST]);
    assert!(probe.is_done());
    assert!(input.is_empty());
    let info = probe.info();
    assert_eq!(info.foreground, Some(Rgb::new(0xdc, 0xdf, 0xe4)));
    assert_eq!(info.background, Some(Rgb::new(0x28, 0x2c, 0x34)));
    assert_eq!(info.cursor, Some(Rgb::new(255, 255, 255)));
    assert_eq!(info.cell_px, Some((10, 20)));
    assert!(info.sync_output);
    assert_eq!(info.kitty_flags, Some(0));
}

#[test]
fn minimal_host_answers_only_da1() {
    let (probe, input) = run(&[b"\x1b[?1;2c"]);
    assert!(probe.is_done());
    assert!(input.is_empty());
    assert_eq!(probe.info(), HostInfo::default());
}

#[test]
fn keys_typed_during_the_probe_are_kept_in_order() {
    let (probe, input) = run(&[b"ab\x1b[A\x1b]11;rgb:0/0/0\x07c\x1bx\x1b[?62c", b"\x1b[B"]);
    assert!(probe.is_done());
    assert_eq!(input, b"ab\x1b[Ac\x1bx\x1b[B");
    assert_eq!(probe.info().background, Some(Rgb::new(0, 0, 0)));
}

#[test]
fn bytes_after_the_terminator_are_input() {
    let (_, input) = run(&[b"\x1b[?62cls\r"]);
    assert_eq!(input, b"ls\r");
}

#[test]
fn abandon_releases_a_partial_reply() {
    let mut probe = HostProbe::new();
    let mut input = Vec::new();
    probe.feed(b"x\x1b]11;rgb:00", &mut input);
    assert!(!probe.is_done());
    assert_eq!(input, b"x");
    probe.abandon(&mut input);
    assert!(probe.is_done());
    assert_eq!(input, b"x\x1b]11;rgb:00");
}

#[test]
fn unterminated_osc_is_not_buffered_forever() {
    let mut long = b"\x1b]11;".to_vec();
    long.extend(std::iter::repeat_n(b'a', 400));
    let (probe, input) = run(&[&long, b"\x1b[?62c"]);
    assert!(probe.is_done());
    assert_eq!(input, &long[..]);
}

proptest! {
    /// Chunk boundaries never change the outcome.
    #[test]
    fn chunking_is_irrelevant(cuts in proptest::collection::vec(0usize..KITTY_HOST.len(), 0..8)) {
        let mut cuts = cuts;
        cuts.sort_unstable();
        cuts.dedup();
        let mut chunks = Vec::new();
        let mut last = 0;
        for cut in cuts {
            chunks.push(&KITTY_HOST[last..cut]);
            last = cut;
        }
        chunks.push(&KITTY_HOST[last..]);
        let (probe, input) = run(&chunks);
        let (whole, _) = run(&[KITTY_HOST]);
        prop_assert!(probe.is_done());
        prop_assert!(input.is_empty());
        prop_assert_eq!(probe.info(), whole.info());
    }
}
