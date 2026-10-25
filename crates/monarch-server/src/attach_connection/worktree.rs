//! Worktree requests from a client: list, make, open, remove. Git runs on
//! a blocking thread; only the asking client hears the answer (new tabs
//! reach everyone through the layout).

use std::sync::Arc;

use monarch_core::ClientId;
use monarch_proto::{NewWorktree, ServerMsg};

use crate::ServerContext;

/// Lists the session's worktrees in the background for client `id`.
pub(super) fn list(ctx: &Arc<ServerContext>, id: ClientId) {
    answer(ctx, id, |ctx| ServerMsg::Worktrees(ctx.worktree_list()));
}

/// Removes the worktree at `path` in the background; tells client `id`.
pub(super) fn remove(ctx: &Arc<ServerContext>, id: ClientId, path: String) {
    answer(ctx, id, move |ctx| {
        let removed = ctx
            .remove_worktree(&path)
            .map(|()| path)
            .map_err(|e| e.to_string());
        ServerMsg::WorktreeRemoved(removed)
    });
}

/// Opens a tab with a shell in the worktree at `path`.
pub(super) fn open(ctx: &ServerContext, path: &str) {
    if let Err(e) = ctx.open_worktree(path) {
        eprintln!("monarch: {e}");
    }
    ctx.wake();
}

/// Makes the worktree in the background and answers client `id`.
pub(super) fn request(ctx: &Arc<ServerContext>, id: ClientId, request: NewWorktree) {
    answer(ctx, id, move |ctx| {
        ServerMsg::WorktreeMade(ctx.new_worktree(&request).map_err(|e| e.to_string()))
    });
}

/// Runs `work` on a blocking thread and sends its message to client `id`.
fn answer(
    ctx: &Arc<ServerContext>,
    id: ClientId,
    work: impl FnOnce(&ServerContext) -> ServerMsg + Send + 'static,
) {
    let ctx = Arc::clone(ctx);
    tokio::task::spawn_blocking(move || {
        let msg = work(&ctx);
        if let Some(client) = ctx.lock().client_mut(id) {
            client.push(vec![msg]);
        }
        ctx.wake();
    });
}
