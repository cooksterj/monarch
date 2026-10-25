//! Answers a client's request to review a pane's checkout, or to open one
//! of its files. Git can take a while on a big repository, so it runs on a
//! blocking thread and never holds up this client's keystrokes or anyone's
//! frames.

use std::sync::Arc;

use monarch_core::{ClientId, PaneId};
use monarch_proto::ServerMsg;

use crate::ServerContext;

/// Reads `pane`'s changes in the background and sends them to the client
/// that asked, and only to it.
pub(super) fn request(ctx: &Arc<ServerContext>, id: ClientId, pane: PaneId) {
    let ctx = Arc::clone(ctx);
    tokio::task::spawn_blocking(move || reply(&ctx, id, pane));
}

/// Opens `path` from `pane`'s checkout in `editor`, zoomed over the
/// focused pane.
pub(super) fn edit(ctx: &ServerContext, pane: PaneId, path: &str, editor: &str) {
    if let Err(e) = ctx.edit(pane, path, editor) {
        eprintln!("monarch: {e}");
    }
    ctx.wake();
}

fn reply(ctx: &ServerContext, id: ClientId, pane: PaneId) {
    let review = ctx.review(pane);
    if let Some(client) = ctx.lock().client_mut(id) {
        client.push(vec![ServerMsg::Review { pane, review }]);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use monarch_proto::{FileChange, PanePlace};
    use tokio::sync::mpsc;

    use super::*;
    use crate::test_support::{FakePane, context};
    use crate::{DiffReader, GitRunner, PaneSpec};

    struct OneEdit;

    impl GitRunner for OneEdit {
        fn run(&self, _dir: &Path, args: &[&str]) -> Option<String> {
            args.contains(&"diff")
                .then(|| "diff --git a/x.rs b/x.rs\n@@ -1 +1 @@\n-a\n+b\n".to_owned())
        }
    }

    #[test]
    fn only_the_asking_client_hears_and_outside_git_is_none() {
        let ctx = context().with_diffs(DiffReader::new(Box::new(OneEdit)));
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let (tx, mut asker) = mpsc::channel(8);
        let (other_tx, mut other) = mpsc::channel(8);
        let id = ctx.lock().add_client(tx);
        ctx.lock().add_client(other_tx);
        while asker.try_recv().is_ok() {}
        while other.try_recv().is_ok() {}
        reply(&ctx, id, pane);
        assert_eq!(
            asker.try_recv().ok(),
            Some(vec![ServerMsg::Review { pane, review: None }])
        );
        let place = PanePlace {
            worktree: Some("/w".into()),
            branch: Some("feat".into()),
            ..PanePlace::outside("/w/src")
        };
        ctx.lock().set_place(pane, Some(place));
        while asker.try_recv().is_ok() {}
        while other.try_recv().is_ok() {}
        reply(&ctx, id, pane);
        let Some(msgs) = asker.try_recv().ok() else {
            panic!("no answer");
        };
        let [
            ServerMsg::Review {
                review: Some(review),
                ..
            },
        ] = msgs.as_slice()
        else {
            panic!("not a review: {msgs:?}");
        };
        assert_eq!(
            (review.root.as_str(), review.branch.as_deref()),
            ("/w", Some("feat"))
        );
        assert_eq!(review.files[0].change, FileChange::Modified);
        assert!(other.try_recv().is_err(), "other clients hear nothing");
    }
}
