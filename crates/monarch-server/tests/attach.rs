//! End to end: a real server, a real PTY, a client speaking the protocol.

use std::path::Path;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use monarch_core::PaneId;
use monarch_proto::{ClientMsg, FrameCodec, Hello, HelloAck, ScreenSize, ServerMsg};
use monarch_server::{PaneSpec, Server, ServerConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::time::timeout;
use tokio_util::codec::{FramedRead, FramedWrite};

type Rx = FramedRead<OwnedReadHalf, FrameCodec<ServerMsg, ClientMsg>>;
type Tx = FramedWrite<OwnedWriteHalf, FrameCodec<ServerMsg, ClientMsg>>;

const WAIT: Duration = Duration::from_secs(10);

async fn connect(socket: &Path) -> (Rx, Tx) {
    let mut stream = timeout(WAIT, async {
        loop {
            if let Ok(s) = UnixStream::connect(socket).await {
                return s;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("server socket");
    let mut hello = serde_json::to_vec(&Hello::attach("test")).expect("json");
    hello.push(b'\n');
    stream.write_all(&hello).await.expect("hello");
    let mut line = Vec::new();
    while let Ok(byte) = stream.read_u8().await {
        if byte == b'\n' {
            break;
        }
        line.push(byte);
    }
    let ack: HelloAck = serde_json::from_slice(&line).expect("ack");
    assert!(ack.ok, "{ack:?}");
    let (rd, wr) = stream.into_split();
    (
        FramedRead::new(rd, FrameCodec::default()),
        FramedWrite::new(wr, FrameCodec::default()),
    )
}

/// Reads until a frame satisfies `pred`, returning that message.
async fn until(rx: &mut Rx, mut pred: impl FnMut(&ServerMsg) -> bool) -> ServerMsg {
    timeout(WAIT, async {
        loop {
            let msg = rx.next().await.expect("stream open").expect("valid frame");
            if pred(&msg) {
                return msg;
            }
        }
    })
    .await
    .expect("expected message in time")
}

fn config(socket: &Path, pane: PaneSpec) -> ServerConfig {
    ServerConfig {
        session: "t".into(),
        socket: Some(socket.into()),
        panes: vec![pane],
        scrollback: 100,
        ..ServerConfig::default()
    }
}

fn shows(msg: &ServerMsg, text: &str) -> bool {
    let runs: Vec<_> = match msg {
        ServerMsg::Snapshot(s) => s.rows.iter().flatten().collect(),
        ServerMsg::Delta(d) => d.rows.iter().flat_map(|r| &r.runs).collect(),
        _ => return false,
    };
    runs.iter().any(|run| run.text.contains(text))
}

#[tokio::test(flavor = "multi_thread")]
async fn attach_type_echo_ping_and_exit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = dir.path().join("t.sock");
    let cat = PaneSpec {
        argv: vec!["cat".into()],
        ..PaneSpec::default()
    };
    let server = tokio::spawn(Server::serve(config(&socket, cat)));
    let (mut rx, mut tx) = connect(&socket).await;

    let layout = until(&mut rx, |m| matches!(m, ServerMsg::Layout(_))).await;
    assert!(matches!(layout, ServerMsg::Layout(l) if l.focus == Some(PaneId(1))));
    tx.send(ClientMsg::Resize(ScreenSize::new(40, 5)))
        .await
        .expect("resize");
    until(
        &mut rx,
        |m| matches!(m, ServerMsg::Snapshot(s) if s.size == ScreenSize::new(40, 5)),
    )
    .await;

    let input = ClientMsg::Input {
        pane: PaneId(1),
        bytes: b"monarch-ok\r".to_vec(),
    };
    tx.send(input).await.expect("input");
    until(&mut rx, |m| shows(m, "monarch-ok")).await;

    tx.send(ClientMsg::Ping(7)).await.expect("ping");
    until(&mut rx, |m| *m == ServerMsg::Pong(7)).await;

    tx.send(ClientMsg::Input {
        pane: PaneId(1),
        bytes: vec![0x04],
    })
    .await
    .expect("eof");
    until(&mut rx, |m| *m == ServerMsg::PaneExited { pane: PaneId(1) }).await;
    timeout(WAIT, server)
        .await
        .expect("server stops")
        .expect("join")
        .expect("clean exit");
    assert!(!socket.exists(), "socket removed on exit");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_server_for_the_same_socket_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let socket = dir.path().join("dup.sock");
    let config = config(&socket, PaneSpec::shell_command("sleep 5"));
    let first = tokio::spawn(Server::serve(config.clone()));
    let _ = connect(&socket).await;
    let second = Server::serve(config).await;
    assert!(matches!(
        second,
        Err(monarch_server::ServerError::AlreadyRunning { .. })
    ));
    first.abort();
}
