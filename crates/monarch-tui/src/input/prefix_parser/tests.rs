//! Unit and property tests for [`PrefixParser`].

use proptest::prelude::*;

use super::*;

const P: u8 = 0x02;

fn parse(chunks: &[&[u8]]) -> Vec<InputEvent> {
    let mut parser = PrefixParser::new(P);
    let mut out = Vec::new();
    for chunk in chunks {
        parser.feed(chunk, &mut out);
    }
    out
}

/// Joins adjacent `Forward`s so results compare independently of chunking.
fn normalise(events: Vec<InputEvent>) -> Vec<InputEvent> {
    let mut out: Vec<InputEvent> = Vec::new();
    for e in events {
        match (out.last_mut(), e) {
            (Some(InputEvent::Forward(a)), InputEvent::Forward(b)) => a.extend(b),
            (_, e) => out.push(e),
        }
    }
    out
}

fn fwd(b: &[u8]) -> InputEvent {
    InputEvent::Forward(b.to_vec())
}

#[test]
fn plain_input_passes_through_untouched() {
    let keys = b"ls -la\r\x1b[A\x1b[1;2A\x1b[13;2u\x1bx\x1b";
    assert_eq!(parse(&[keys]), [fwd(keys)]);
}

#[test]
fn prefix_and_command_in_one_read() {
    assert_eq!(
        parse(&[b"ab\x02qcd"]),
        [fwd(b"ab"), InputEvent::Command(b'q'), fwd(b"cd")]
    );
}

#[test]
fn prefix_and_command_in_separate_reads() {
    let mut parser = PrefixParser::new(P);
    let mut out = Vec::new();
    parser.feed(b"\x02", &mut out);
    assert!(parser.awaiting_command());
    parser.feed(b"%", &mut out);
    assert_eq!(out, [InputEvent::Command(b'%')]);
}

#[test]
fn double_prefix_sends_one_literal_prefix() {
    assert_eq!(normalise(parse(&[b"\x02\x02x"])), [fwd(b"\x02x")]);
}

#[test]
fn prefix_inside_bracketed_paste_belongs_to_the_pane() {
    let paste = b"\x1b[200~a\x02q\x1b[201~";
    assert_eq!(parse(&[paste]), [fwd(paste)]);
    // After the paste ends, the prefix works again.
    let after = parse(&[paste, b"\x02d"]);
    assert_eq!(after.last(), Some(&InputEvent::Command(b'd')));
}

#[test]
fn paste_markers_split_across_reads() {
    let out = normalise(parse(&[b"\x1b[20", b"0~x\x02", b"y\x1b[2", b"01~\x02z"]));
    assert_eq!(
        out,
        [fwd(b"\x1b[200~x\x02y\x1b[201~"), InputEvent::Command(b'z')]
    );
}

#[test]
fn kitty_encoded_prefix_and_command() {
    assert_eq!(
        parse(&[b"a\x1b[98;5uqb"]),
        [fwd(b"a"), InputEvent::Command(b'q'), fwd(b"b")]
    );
}

#[test]
fn kitty_command_key_and_releases_are_consumed() {
    // Flags 1|2|8: every key is a report, releases included.
    let keys: &[&[u8]] = &[
        b"\x1b[98;5u",
        b"\x1b[113u",
        b"\x1b[113;1:3u",
        b"\x1b[98;5:3u",
        b"\x1b[120u",
    ];
    assert_eq!(
        normalise(parse(keys)),
        [InputEvent::Command(b'q'), fwd(b"\x1b[120u")]
    );
}

#[test]
fn kitty_double_prefix_sends_the_report_to_the_pane() {
    assert_eq!(
        normalise(parse(&[b"\x1b[98;5u\x1b[98;5u"])),
        [fwd(b"\x1b[98;5u")]
    );
}

#[test]
fn other_kitty_keys_pass_through() {
    let keys = b"\x1b[13;2u\x1b[98;7u\x1b[97;5:3u";
    assert_eq!(parse(&[keys]), [fwd(keys)]);
}

#[test]
fn kitty_prefix_inside_paste_belongs_to_the_pane() {
    let paste = b"\x1b[200~\x1b[98;5uq\x1b[201~";
    assert_eq!(parse(&[paste]), [fwd(paste)]);
}

proptest! {
    /// Results never depend on how the host split input into reads.
    #[test]
    fn chunking_is_irrelevant(
        bytes in prop::collection::vec(prop_oneof![
            4 => any::<u8>(),
            1 => Just(P),
            1 => Just(0x1b),
        ], 0..64),
        cuts in prop::collection::vec(any::<prop::sample::Index>(), 0..5),
    ) {
        let mut points: Vec<usize> = cuts.iter().map(|c| c.index(bytes.len() + 1)).collect();
        points.sort_unstable();
        let mut chunks = Vec::new();
        let mut start = 0;
        for p in points {
            chunks.push(&bytes[start..p]);
            start = p;
        }
        chunks.push(&bytes[start..]);
        prop_assert_eq!(normalise(parse(&chunks)), normalise(parse(&[&bytes])));
    }

    /// Input without the prefix byte is forwarded byte-for-byte.
    #[test]
    fn lossless_without_prefix(bytes in prop::collection::vec(any::<u8>().prop_filter("no prefix", |b| *b != P), 0..64)) {
        let expected = if bytes.is_empty() { vec![] } else { vec![fwd(&bytes)] };
        prop_assert_eq!(normalise(parse(&[&bytes])), expected);
    }
}
