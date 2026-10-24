//! Tests for [`FrameCodec`], including full-message round trips.

use monarch_core::PaneId;
use proptest::prelude::*;

use super::*;
use crate::attach::{ClientMsg, Delta, PaneState, RowDelta, Run, ServerMsg, StyleDef, StyleId};
use crate::{CellAttrs, CellStyle, Rgb, ScreenSize, TermColor};

type ServerSide = FrameCodec<ClientMsg, ServerMsg>;
type ClientSide = FrameCodec<ServerMsg, ClientMsg>;

fn delta() -> ServerMsg {
    ServerMsg::Delta(Delta {
        pane: PaneId(3),
        generation: 1,
        seq: 42,
        rows: vec![RowDelta {
            line: 2,
            runs: vec![Run {
                style: StyleId(1),
                width: 1,
                cells: 5,
                text: "hello".into(),
            }],
        }],
        state: PaneState::default(),
    })
}

fn style_def() -> ServerMsg {
    ServerMsg::DefineStyle(StyleDef {
        id: StyleId(1),
        style: CellStyle {
            fg: TermColor::Rgb(Rgb::new(1, 2, 3)),
            bg: TermColor::Indexed(4),
            underline: Some(TermColor::Default),
            attrs: CellAttrs::BOLD | CellAttrs::UNDERCURL,
        },
    })
}

#[test]
fn server_messages_round_trip() {
    let server = ServerSide::default();
    let client = ClientSide::default();
    let mut wire = Vec::new();
    for msg in [
        style_def(),
        delta(),
        ServerMsg::Pong(7),
        ServerMsg::HostWrite(vec![7]),
    ] {
        server.encode_to(&msg, &mut wire).expect("encode");
    }
    let mut buf = BytesMut::from(&wire[..]);
    let mut got = Vec::new();
    while let Some(msg) = client.decode_from(&mut buf).expect("decode") {
        got.push(msg);
    }
    assert_eq!(
        got,
        [
            style_def(),
            delta(),
            ServerMsg::Pong(7),
            ServerMsg::HostWrite(vec![7])
        ]
    );
    assert!(buf.is_empty());
}

#[test]
fn client_messages_round_trip_through_tokio_traits() {
    let mut client = ClientSide::default();
    let mut server = ServerSide::default();
    let msgs = [
        ClientMsg::Input {
            pane: PaneId(1),
            bytes: b"\x1b[200~x\x1b[201~".to_vec(),
        },
        ClientMsg::Resize(ScreenSize::new(120, 40)),
        ClientMsg::Scroll {
            pane: PaneId(1),
            lines: -3,
        },
        ClientMsg::Detach,
    ];
    let mut buf = BytesMut::new();
    for msg in msgs.clone() {
        client.encode(msg, &mut buf).expect("encode");
    }
    for expected in msgs {
        assert_eq!(server.decode(&mut buf).expect("decode"), Some(expected));
    }
    assert_eq!(server.decode(&mut buf).expect("decode"), None);
}

#[test]
fn partial_frames_wait_for_more_bytes() {
    let mut wire = Vec::new();
    ServerSide::default()
        .encode_to(&delta(), &mut wire)
        .expect("encode");
    let client = ClientSide::default();
    let mut buf = BytesMut::from(&wire[..wire.len() - 1]);
    assert!(client.decode_from(&mut buf).expect("partial").is_none());
    buf.extend_from_slice(&wire[wire.len() - 1..]);
    assert_eq!(client.decode_from(&mut buf).expect("whole"), Some(delta()));
}

#[test]
fn oversized_frames_are_rejected_both_ways() {
    let small = FrameCodec::<ServerMsg, ServerMsg>::new(8);
    let err = small
        .encode_to(&delta(), &mut Vec::new())
        .expect_err("too big to send");
    assert!(matches!(err, ProtoError::FrameTooLarge { max: 8, .. }));
    let mut buf = BytesMut::from(&[0u8, 0, 1, 0][..]);
    assert!(matches!(
        small.decode_from(&mut buf),
        Err(ProtoError::FrameTooLarge { len: 256, .. })
    ));
}

#[test]
fn garbage_bodies_are_decode_errors() {
    let mut buf = BytesMut::from(&[0u8, 0, 0, 2, 0xff, 0xff][..]);
    assert!(matches!(
        ClientSide::default().decode_from(&mut buf),
        Err(ProtoError::Decode(_))
    ));
}

proptest! {
    /// Arbitrary input bytes survive the wire however they are split.
    #[test]
    fn input_bytes_survive_any_split(bytes in proptest::collection::vec(any::<u8>(), 0..512), cut in 0usize..600) {
        let msg = ClientMsg::Input { pane: PaneId(9), bytes };
        let mut wire = Vec::new();
        ClientSide::default().encode_to(&msg, &mut wire).expect("encode");
        let cut = cut.min(wire.len());
        let server = ServerSide::default();
        let mut buf = BytesMut::from(&wire[..cut]);
        let early = server.decode_from(&mut buf).expect("decode");
        buf.extend_from_slice(&wire[cut..]);
        let got = early.or_else(|| server.decode_from(&mut buf).expect("decode"));
        prop_assert_eq!(got, Some(msg));
    }
}
