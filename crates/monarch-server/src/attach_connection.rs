//! One client connection: handshake, then attach frames both ways.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use monarch_core::ClientId;
use monarch_proto::{ClientMsg, FrameCodec, Hello, HelloAck, ServerMsg};
use monarch_term::ScrollRequest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc::{self, Receiver};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::{RENDER_LANE, ServerContext, ServerError};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Longest handshake line accepted.
const MAX_HELLO: usize = 4096;

type Codec = FrameCodec<ClientMsg, ServerMsg>;

/// Serves one accepted socket until the client detaches or disconnects.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttachConnection;

impl AttachConnection {
    /// Runs the connection to completion.
    ///
    /// # Errors
    /// The handshake or the stream failed.
    pub async fn serve(ctx: Arc<ServerContext>, mut stream: UnixStream) -> Result<(), ServerError> {
        let io = |e| ServerError::io("attach handshake", e);
        let hello = read_hello(&mut stream).await.map_err(io)?;
        let ack = if hello.as_ref().is_some_and(Hello::is_attach) {
            HelloAck::accept(VERSION)
        } else {
            HelloAck::refuse(VERSION, "only the attach protocol is available")
        };
        write_line(&mut stream, &ack).await.map_err(io)?;
        if !ack.ok {
            return Ok(());
        }
        let (rd, wr) = stream.into_split();
        let (lane, lane_rx) = mpsc::channel(RENDER_LANE);
        let id = ctx.lock().add_client(lane);
        ctx.wake();
        let writer = tokio::spawn(write_frames(wr, lane_rx));
        let result = read_messages(&ctx, id, rd).await;
        ctx.lock().remove_client(id);
        writer.abort();
        result
    }
}

/// Reads the JSON hello line byte by byte, so nothing after it is consumed.
async fn read_hello(stream: &mut UnixStream) -> std::io::Result<Option<Hello>> {
    let mut line = Vec::new();
    loop {
        let byte = stream.read_u8().await?;
        if byte == b'\n' {
            break;
        }
        line.push(byte);
        if line.len() > MAX_HELLO {
            return Ok(None);
        }
    }
    Ok(serde_json::from_slice(&line).ok())
}

async fn write_line(stream: &mut UnixStream, ack: &HelloAck) -> std::io::Result<()> {
    let mut line = serde_json::to_vec(ack).map_err(std::io::Error::other)?;
    line.push(b'\n');
    stream.write_all(&line).await
}

/// Drains the client's render lane onto the socket, one flush per frame.
async fn write_frames(wr: OwnedWriteHalf, mut lane: Receiver<Vec<ServerMsg>>) {
    let mut sink = FramedWrite::new(wr, Codec::default());
    while let Some(frame) = lane.recv().await {
        for msg in frame {
            if sink.feed(msg).await.is_err() {
                return;
            }
        }
        if sink.flush().await.is_err() {
            return;
        }
    }
}

async fn read_messages(
    ctx: &ServerContext,
    id: ClientId,
    rd: OwnedReadHalf,
) -> Result<(), ServerError> {
    let mut frames = FramedRead::new(rd, Codec::default());
    while let Some(msg) = frames.next().await {
        let msg = msg.map_err(|e| ServerError::io("attach stream", std::io::Error::other(e)))?;
        if !handle(ctx, id, msg) {
            break;
        }
    }
    Ok(())
}

/// Applies one client message; `false` ends the connection.
fn handle(ctx: &ServerContext, id: ClientId, msg: ClientMsg) -> bool {
    let mut state = ctx.lock();
    match msg {
        ClientMsg::Input { pane, bytes } => {
            if let Some(io) = state.pane(pane) {
                io.send(bytes);
            }
        }
        ClientMsg::Resize(size) => state.resize(size),
        ClientMsg::HostInfo(info) => state.set_host(info),
        ClientMsg::Focus(pane) => {
            state.set_focus(pane);
        }
        ClientMsg::Scroll { pane, lines } => {
            if let Some(io) = state.pane(pane) {
                io.shared().lock().scroll(ScrollRequest::Lines(lines));
                io.shared().mark_dirty();
            }
        }
        ClientMsg::Resync(pane) => {
            if let Some(client) = state.client_mut(id) {
                client.mark_stale(pane);
            }
        }
        ClientMsg::Ping(token) => {
            if let Some(client) = state.client_mut(id) {
                client.push(vec![ServerMsg::Pong(token)]);
            }
            return true;
        }
        ClientMsg::Detach => return false,
        _ => return true,
    }
    drop(state);
    ctx.wake();
    true
}
