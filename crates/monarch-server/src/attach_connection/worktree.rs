//! Makes a worktree a client asked for. Git checks it out on a blocking
//! thread; only the asking client hears how it went (the new tab reaches
//! everyone through the layout).

use std::sync::Arc;

use monarch_core::ClientId;
use monarch_proto::{NewWorktree, ServerMsg};

use crate::{ServerContext, ServerError};

/// Makes the worktree in the background and answers client `id`.
pub(super) fn request(ctx: &Arc<ServerContext>, id: ClientId, request: NewWorktree) {
    let ctx = Arc::clone(ctx);
    tokio::task::spawn_blocking(move || {
        // The form already says what failed; send just why.
        let made = ctx.new_worktree(&request).map_err(|e| match e {
            ServerError::Worktree(why) => why,
            other => other.to_string(),
        });
        if let Some(client) = ctx.lock().client_mut(id) {
            client.push(vec![ServerMsg::WorktreeMade(made)]);
        }
        ctx.wake();
    });
}
