//! Answers the file viewer: a pane's checkout's files, and one file read
//! and highlighted. Git, the disk and highlighting can each take a while,
//! so they run on a blocking thread, never holding up keystrokes or
//! frames, and the answer goes only to the client that asked.

use std::sync::Arc;

use monarch_core::{ClientId, PaneId};
use monarch_proto::{ClientMsg, GridPoint, SelectShape, ServerMsg};

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

/// Opens the file the selection from `start` to `end` in `pane` names, in
/// the background; when it names none, sends its text back instead, for
/// the client to search the checkout with.
pub(super) fn view_selected(ctx: &Arc<ServerContext>, id: ClientId, at: SelectedRange) {
    let ctx = Arc::clone(ctx);
    tokio::task::spawn_blocking(move || {
        let SelectedRange { pane, start, end } = at;
        let text = {
            let state = ctx.lock();
            let Some(io) = state.pane(pane) else {
                return;
            };
            io.shared()
                .lock()
                .text_range(start, end, SelectShape::Chars)
        };
        let msg = match ctx.open_named(pane, &text) {
            Ok((file, line)) => ServerMsg::ViewFile { pane, file, line },
            Err(_) => ServerMsg::NotAFile {
                pane,
                text: ServerContext::search_text(&text),
            },
        };
        answer(&ctx, id, msg);
    });
}

/// Sends the text from `start` to `end` in `pane` back to quote to an
/// agent, in the background, with the lines of the checkout's file it
/// shows when it is found there once.
pub(super) fn quote(ctx: &Arc<ServerContext>, id: ClientId, msg: &ClientMsg) {
    let ClientMsg::Quote {
        pane,
        start,
        end,
        shape,
    } = *msg
    else {
        return;
    };
    let at = SelectedRange { pane, start, end };
    let ctx = Arc::clone(ctx);
    tokio::task::spawn_blocking(move || {
        if let Some(msg) = quoted(&ctx, at, shape) {
            answer(&ctx, id, msg);
        }
    });
}

/// The answer to a quote; `None` when the pane is gone. The pane is
/// locked only while its text is read, git runs after.
fn quoted(ctx: &ServerContext, at: SelectedRange, shape: SelectShape) -> Option<ServerMsg> {
    let SelectedRange { pane, start, end } = at;
    let text = {
        let state = ctx.lock();
        let io = state.pane(pane)?;
        io.shared().lock().text_range(start, end, shape)
    };
    let source = ctx.locate(pane, &text);
    Some(ServerMsg::Quote { pane, text, source })
}

/// A selection in a pane.
#[derive(Debug, Clone, Copy)]
pub(super) struct SelectedRange {
    /// The pane.
    pub pane: PaneId,
    /// One end.
    pub start: GridPoint,
    /// The other, inclusive.
    pub end: GridPoint,
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
    fn quotes_come_back_as_text_and_gone_panes_get_no_answer() {
        let ctx = context();
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        {
            let state = ctx.lock();
            let io = state.pane(pane).expect("pane");
            io.shared().lock().feed(b"hello world", &mut Vec::new());
        }
        let at = |pane| SelectedRange {
            pane,
            start: GridPoint { line: 0, col: 0 },
            end: GridPoint { line: 0, col: 4 },
        };
        assert_eq!(quoted(&ctx, at(PaneId(99)), SelectShape::Chars), None);
        let text = "hello".to_owned();
        let source = None;
        let got = quoted(&ctx, at(pane), SelectShape::Chars);
        assert_eq!(
            got,
            Some(ServerMsg::Quote { pane, text, source }),
            "not in a checkout"
        );
    }

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
