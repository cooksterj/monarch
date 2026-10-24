//! Unit tests and a property test that checks the tap against `vte` itself.

use alacritty_terminal::vte::{Params, Parser, Perform};
use proptest::prelude::*;

use super::*;

fn scan_all(chunks: &[&[u8]]) -> Vec<Osc> {
    let mut tap = OscTap::new();
    let mut out = Vec::new();
    for chunk in chunks {
        tap.scan(chunk, &mut out);
    }
    out
}

fn osc(number: u32, payload: &str) -> Osc {
    Osc {
        number,
        payload: payload.as_bytes().to_vec(),
    }
}

#[test]
fn captures_bel_and_st_terminated() {
    let out = scan_all(&[b"hi\x1b]9;done\x07 there \x1b]777;notify;T;B\x1b\\!"]);
    assert_eq!(out, [osc(9, "done"), osc(777, "notify;T;B")]);
}

#[test]
fn survives_any_chunk_split() {
    let stream: &[u8] = b"ab\x1b]133;A\x07cd\x1b]7;file://h/tmp\x1b\\";
    for cut in 0..stream.len() {
        let (a, b) = stream.split_at(cut);
        assert_eq!(
            scan_all(&[a, b]),
            [osc(133, "A"), osc(7, "file://h/tmp")],
            "cut {cut}"
        );
    }
}

#[test]
fn ignores_uncaptured_numbers_and_other_sequences() {
    let out = scan_all(&[b"\x1b]0;title\x07\x1b[31mred\x1b[0m\x1b]52;c;eA==\x07"]);
    assert!(out.is_empty());
}

#[test]
fn can_and_sub_end_the_string_like_vte() {
    assert_eq!(scan_all(&[b"\x1b]9;a\x18"]), [osc(9, "a")]);
    assert_eq!(scan_all(&[b"\x1b]9;b\x1a"]), [osc(9, "b")]);
}

#[test]
fn drops_oversized_bodies() {
    let mut big = b"\x1b]9;".to_vec();
    big.extend(std::iter::repeat_n(b'x', MAX_BODY + 10));
    big.extend(b"\x07\x1b]9;ok\x07");
    assert_eq!(scan_all(&[&big]), [osc(9, "ok")]);
}

/// Collects what vte dispatches, filtered and shaped like the tap's output.
#[derive(Default)]
struct Oracle(Vec<Osc>);

impl Perform for Oracle {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let body = params.join(&b';');
        self.0.extend(Osc::parse(&body));
    }
    fn csi_dispatch(&mut self, _: &Params, _: &[u8], _: bool, _: char) {}
}

fn vte_oscs(bytes: &[u8]) -> Vec<Osc> {
    let mut parser: Parser = Parser::new();
    let mut oracle = Oracle::default();
    parser.advance(&mut oracle, bytes);
    oracle.0
}

/// Terminal-ish byte streams: text, CSI/SGR, OSCs with every terminator,
/// stray escapes and control bytes.
fn fragment() -> impl Strategy<Value = Vec<u8>> {
    let number = prop::sample::select(vec!["0", "2", "7", "9", "52", "99", "133", "777", "7501"]);
    let payload = prop::collection::vec(
        prop_oneof![Just(b';'), 0x20u8..0x7f, 0x01u8..0x20, 0x80u8..=0xff],
        0..24,
    );
    let term = prop::sample::select(vec![&b"\x07"[..], b"\x1b\\", b"\x18", b"\x1a", b"\x1b[0m"]);
    let osc = (number, payload, term).prop_map(|(n, mut p, t)| {
        p.retain(|&b| b != 0x1b);
        let semis = p.iter().filter(|&&b| b == b';').count();
        if semis > 10 {
            p.retain(|&b| b != b';');
        }
        [b"\x1b]".as_slice(), n.as_bytes(), b";", &p, t].concat()
    });
    prop_oneof![
        3 => osc,
        2 => "[a-z ]{0,12}".prop_map(String::into_bytes),
        1 => Just(b"\x1b[38;5;196m".to_vec()),
        1 => Just(b"\x1b".to_vec()),
        1 => prop::collection::vec(any::<u8>(), 0..6),
    ]
}

proptest! {
    #[test]
    fn matches_vte_on_any_chunking(
        frags in prop::collection::vec(fragment(), 0..12),
        cuts in prop::collection::vec(any::<prop::sample::Index>(), 0..4),
    ) {
        let stream = frags.concat();
        let mut points: Vec<usize> = cuts.iter().map(|c| c.index(stream.len() + 1)).collect();
        points.sort_unstable();
        let mut chunks = Vec::new();
        let mut start = 0;
        for p in points {
            chunks.push(&stream[start..p]);
            start = p;
        }
        chunks.push(&stream[start..]);
        prop_assert_eq!(scan_all(&chunks), vte_oscs(&stream));
    }
}
