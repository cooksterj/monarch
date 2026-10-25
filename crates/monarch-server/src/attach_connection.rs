//! An attach connection: frames both ways once the handshake is done.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use monarch_core::{Action, ClientId, PaneId};
use monarch_proto::{ClientMsg, FrameCodec, ServerMsg};
use monarch_term::ScrollRequest;
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc::{self, Receiver};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::{ActionEffect, RENDER_LANE, ServerContext, ServerError, SessionState};

mod history;
mod review;
mod worktree;

type Codec = FrameCodec<ClientMsg, ServerMsg>;

/// Serves an attached client until it detaches or disconnects.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttachConnection;

impl AttachConnection {
    /// Runs the attach stream, after the handshake, to completion.
    ///
    /// # Errors
    /// The stream failed.
    pub async fn serve(ctx: Arc<ServerContext>, stream: UnixStream) -> Result<(), ServerError> {
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
    ctx: &Arc<ServerContext>,
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
fn handle(ctx: &Arc<ServerContext>, id: ClientId, msg: ClientMsg) -> bool {
    match msg {
        ClientMsg::Detach => return false,
        ClientMsg::Action(action) => run_action(ctx, action),
        msg @ (ClientMsg::Search { .. } | ClientMsg::Copy { .. }) => history::reply(ctx, id, msg),
        ClientMsg::Review(pane) => review::request(ctx, id, pane),
        ClientMsg::Edit { pane, path, editor } => review::edit(ctx, pane, &path, &editor),
        ClientMsg::NewWorktree(request) => worktree::request(ctx, id, request),
        msg => apply(ctx, id, msg),
    }
    true
}

/// Applies a message that changes the session or this client.
fn apply(ctx: &ServerContext, id: ClientId, msg: ClientMsg) {
    let mut state = ctx.lock();
    match msg {
        ClientMsg::Input { pane, bytes } => {
            state.answered(pane, &bytes);
            if let Some(io) = state.pane(pane) {
                io.send(bytes);
            }
        }
        ClientMsg::Resize(size) => state.resize_client(id, size),
        ClientMsg::HostInfo(info) => state.set_host(info),
        ClientMsg::Focus(pane) => {
            let before = state.focus();
            state.set_focus(pane);
            saw_new_focus(&mut state, before);
        }
        ClientMsg::Scroll { pane, lines } => scroll(&state, pane, ScrollRequest::Lines(lines)),
        ClientMsg::ScrollTo { pane, top } => scroll(&state, pane, ScrollRequest::TopLine(top)),
        ClientMsg::Resync(pane) => {
            if let Some(client) = state.client_mut(id) {
                client.mark_stale(pane);
            }
        }
        ClientMsg::Ping(token) => {
            if let Some(client) = state.client_mut(id) {
                client.push(vec![ServerMsg::Pong(token)]);
            }
            return;
        }
        _ => return,
    }
    drop(state);
    ctx.wake();
}

fn scroll(state: &crate::SessionState, pane: monarch_core::PaneId, request: ScrollRequest) {
    if let Some(io) = state.pane(pane) {
        io.shared().lock().scroll(request);
        io.shared().mark_dirty();
    }
}

/// Applies a session command, starting a shell when it asks for one.
/// Focus moved from `before`: the pane now focused has been seen.
fn saw_new_focus(state: &mut SessionState, before: Option<PaneId>) {
    if let Some(now) = state.focus().filter(|p| Some(*p) != before) {
        state.saw(now);
    }
}

fn run_action(ctx: &ServerContext, action: Action) {
    let effect = {
        let mut state = ctx.lock();
        let before = state.focus();
        let effect = state.apply_action(action);
        saw_new_focus(&mut state, before);
        effect
    };
    if let ActionEffect::Spawn(placement) = effect
        && let Err(e) = ctx.spawn_shell_at(placement)
    {
        eprintln!("monarch: {e}");
    }
    ctx.wake();
}
