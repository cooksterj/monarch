//! A client asking why a pane's agent is in its state: the explanation
//! runs the agent's manifest over the screen, so it goes on a blocking
//! thread, and only the asking client hears the answer.

use std::sync::Arc;

use monarch_core::{ClientId, PaneId};
use monarch_proto::ServerMsg;

use super::worktree::answer;
use crate::ServerContext;

/// Explains `pane` in the background for client `id`.
pub(super) fn request(ctx: &Arc<ServerContext>, id: ClientId, pane: PaneId) {
    answer(ctx, id, move |ctx| {
        let explained = ctx
            .explain(Some(pane))
            .and_then(|mut panes| panes.pop())
            .map(Box::new);
        ServerMsg::Explained { pane, explained }
    });
}
