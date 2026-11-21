//! Answers the file viewer: a pane's checkout's files, and one file read
//! and highlighted. Git, the disk and highlighting can each take a while,
//! so they run on a blocking thread, never holding up keystrokes or
//! frames, and the answer goes only to the client that asked.

use std::sync::Arc;

use monarch_core::{ClientId, PaneId};
use monarch_proto::ServerMsg;

use crate::ServerContext;

/// Lists `pane`'s checkout's files in the background.
pub(super) fn list(ctx: &Arc<ServerContext>, id: ClientId, pane: PaneId) {
    let ctx = Arc::clone(ctx);
    tokio::task::spawn_blocking(move || {
        let files = ctx.list_files(pane);
        answer(&ctx, id, ServerMsg::Files { pane, files });
    });
}

/// Reads and highlights `path` from `pane`'s checkout in the background.
pub(super) fn open(ctx: &Arc<ServerContext>, id: ClientId, pane: PaneId, path: String) {
    let ctx = Arc::clone(ctx);
    tokio::task::spawn_blocking(move || {
        let file = ctx.open_file(pane, &path);
        answer(&ctx, id, ServerMsg::FileOpened { pane, file });
    });
}

fn answer(ctx: &ServerContext, id: ClientId, msg: ServerMsg) {
    if let Some(client) = ctx.lock().client_mut(id) {
        client.push(vec![msg]);
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;
    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    #[test]
    fn answers_go_only_to_the_asking_client() {
        let ctx = context();
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let (tx, mut asker) = mpsc::channel(8);
        let (other_tx, mut other) = mpsc::channel(8);
        let id = ctx.lock().add_client(tx);
        ctx.lock().add_client(other_tx);
        while asker.try_recv().is_ok() {}
        while other.try_recv().is_ok() {}
        let files = Err("none".to_owned());
        answer(
            &ctx,
            id,
            ServerMsg::Files {
                pane,
                files: files.clone(),
            },
        );
        assert_eq!(
            asker.try_recv().ok(),
            Some(vec![ServerMsg::Files { pane, files }])
        );
        assert!(other.try_recv().is_err());
    }
}
