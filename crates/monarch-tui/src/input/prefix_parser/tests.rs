//! Unit and property tests for [`PrefixParser`].

use proptest::prelude::*;

use std::time::{Duration, Instant};

use monarch_core::Direction;

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

fn cmd(c: char) -> InputEvent {
    InputEvent::Command(Key::Char(c))
}

fn arrow(d: Direction) -> InputEvent {
    InputEvent::Command(Key::Arrow(d))
}

#[test]
fn prefix_then_arrow_is_a_command_not_pane_input() {
    assert_eq!(parse(&[b"\x02\x1b[D"]), [arrow(Direction::Left)]);
    assert_eq!(parse(&[b"\x02\x1bOA"]), [arrow(Direction::Up)]);
    assert_eq!(
        parse(&[b"\x02\x1b[1;5C", b"x"]),
        [arrow(Direction::Right), fwd(b"x")]
    );
    assert_eq!(parse(&[b"\x02\x1b[", b"B"]), [arrow(Direction::Down)]);
}

#[test]
fn prefix_then_esc_alone_is_the_esc_key() {
    assert_eq!(
        parse(&[b"\x02\x1b", b"a"]),
        [InputEvent::Command(Key::Esc), fwd(b"a")]
    );
    assert_eq!(
        parse(&[b"\x02\x1b[98;1:1u"]).len(),
        1,
        "a kitty key is one command"
    );
    assert_eq!(parse(&[b"\x02\x1b[27u"]), [InputEvent::Command(Key::Esc)]);
}

#[test]
fn unbound_sequences_after_the_prefix_are_swallowed() {
    assert_eq!(parse(&[b"\x02\x1b[15~a"]), [fwd(b"a")], "F5");
    assert_eq!(parse(&[b"\x02\x1bxa"]), [fwd(b"a")], "alt+x");
    assert_eq!(parse(&[b"\x02\x01a"]), [fwd(b"a")], "ctrl+a");
}

#[test]
fn a_mouse_report_after_the_prefix_reaches_the_pane_and_the_prefix_waits() {
    let mut parser = PrefixParser::new(P);
    let mut out = Vec::new();
    parser.feed(b"\x02\x1b[<35;4;4M", &mut out);
    assert_eq!(out, [fwd(b"\x1b[<35;4;4M")]);
    assert!(parser.awaiting_command());
    parser.feed(b"z", &mut out);
    assert_eq!(out.last(), Some(&cmd('z')));
}

#[test]
fn plain_input_passes_through_untouched() {
    let keys = b"ls -la\r\x1b[A\x1b[1;2A\x1b[13;2u\x1bx\x1b";
    assert_eq!(parse(&[keys]), [fwd(keys)]);
}

#[test]
fn prefix_and_command_in_one_read() {
    assert_eq!(parse(&[b"ab\x02qcd"]), [fwd(b"ab"), cmd('q'), fwd(b"cd")]);
}

#[test]
fn prefix_and_command_in_separate_reads() {
    let mut parser = PrefixParser::new(P);
    let mut out = Vec::new();
    parser.feed(b"\x02", &mut out);
    assert!(parser.awaiting_command());
    parser.feed(b"%", &mut out);
    assert_eq!(out, [cmd('%')]);
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
    assert_eq!(after.last(), Some(&cmd('d')));
}

#[test]
fn paste_markers_split_across_reads() {
    let out = normalise(parse(&[b"\x1b[20", b"0~x\x02", b"y\x1b[2", b"01~\x02z"]));
    assert_eq!(out, [fwd(b"\x1b[200~x\x02y\x1b[201~"), cmd('z')]);
}

#[test]
fn kitty_encoded_prefix_and_command() {
    assert_eq!(parse(&[b"a\x1b[98;5uqb"]), [fwd(b"a"), cmd('q'), fwd(b"b")]);
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
    assert_eq!(normalise(parse(keys)), [cmd('q'), fwd(b"\x1b[120u")]);
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

fn repeating() -> PrefixParser {
    let keys = [Key::Char('H'), Key::Arrow(Direction::Up)];
    PrefixParser::new(P).with_repeat(keys, Duration::from_millis(500))
}

fn feed(parser: &mut PrefixParser, bytes: &[u8], at: Instant) -> Vec<InputEvent> {
    let mut out = Vec::new();
    parser.feed_at(bytes, at, &mut out);
    out
}

#[test]
fn repeatable_keys_repeat_without_the_prefix_inside_the_window() {
    let t = Instant::now();
    let ms = Duration::from_millis;
    let mut parser = repeating();
    assert_eq!(feed(&mut parser, b"\x02H", t), [cmd('H')]);
    assert_eq!(feed(&mut parser, b"H", t + ms(300)), [cmd('H')]);
    assert_eq!(
        feed(&mut parser, b"HH", t + ms(700)),
        [cmd('H'), cmd('H')],
        "each use extends it"
    );
    assert_eq!(
        feed(&mut parser, b"H", t + ms(1300)),
        [fwd(b"H")],
        "window closed"
    );
}

#[test]
fn other_input_closes_the_window_and_reaches_the_pane() {
    let t = Instant::now();
    let mut parser = repeating();
    feed(&mut parser, b"\x02H", t);
    assert_eq!(feed(&mut parser, b"xH", t), [fwd(b"xH")]);
}

#[test]
fn non_repeatable_commands_open_no_window() {
    let t = Instant::now();
    let mut parser = repeating();
    feed(&mut parser, b"\x02h", t);
    assert_eq!(feed(&mut parser, b"H", t), [fwd(b"H")]);
}

#[test]
fn arrows_and_kitty_keys_repeat_and_the_prefix_still_works() {
    let t = Instant::now();
    let mut parser = repeating();
    feed(&mut parser, b"\x02\x1b[A", t);
    assert_eq!(feed(&mut parser, b"\x1b[A", t), [arrow(Direction::Up)]);
    feed(&mut parser, b"\x02H", t);
    assert_eq!(
        feed(&mut parser, b"\x1b[104:72;2u\x1b[104:72;2:3u", t),
        [cmd('H')],
        "release swallowed"
    );
    assert_eq!(feed(&mut parser, b"\x02d", t), [cmd('d')]);
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
        // Hosts write each escape sequence whole, so no read ends on an ESC.
        let mut points: Vec<usize> = cuts
            .iter()
            .map(|c| c.index(bytes.len() + 1))
            .filter(|&p| p == 0 || bytes[p - 1] != 0x1b)
            .collect();
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
