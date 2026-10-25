//! Requests that read a pane's whole grid for one client: search and copy
//! (copy mode, and mouse selections that reach into scrollback).

use monarch_core::ClientId;
use monarch_proto::{ClientMsg, ServerMsg};

use crate::ServerContext;

/// Answers a `Search` or `Copy` to the client that asked, and only to it.
/// The pane is locked only while it is read.
pub(super) fn reply(ctx: &ServerContext, id: ClientId, msg: ClientMsg) {
    let mut state = ctx.lock();
    let answer = match msg {
        ClientMsg::Search {
            pane,
            query,
            from,
            backward,
        } => {
            let found = state
                .pane(pane)
                .and_then(|io| io.shared().lock().search(&query, from, backward));
            ServerMsg::SearchResult { pane, found }
        }
        ClientMsg::Copy {
            pane,
            start,
            end,
            shape,
        } => {
            let Some(io) = state.pane(pane) else {
                return;
            };
            let text = io.shared().lock().text_range(start, end, shape);
            ServerMsg::Clipboard(text)
        }
        _ => return,
    };
    if let Some(client) = state.client_mut(id) {
        client.push(vec![answer]);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{GridPoint, SelectShape};
    use tokio::sync::mpsc;

    use super::*;
    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    fn at(line: i64, col: u16) -> GridPoint {
        GridPoint { line, col }
    }

    fn with_text(ctx: &crate::ServerContext, text: &[u8]) -> PaneId {
        let pane = ctx.insert_for_test(PaneSpec::shell_command("sh"), FakePane::shell());
        let state = ctx.lock();
        let io = state.pane(pane).expect("pane");
        io.shared().lock().feed(text, &mut Vec::new());
        pane
    }

    #[test]
    fn search_and_copy_answer_only_the_asking_client() {
        let ctx = context();
        let pane = with_text(&ctx, b"hello world\r\nbye");
        let (tx, mut asker) = mpsc::channel(8);
        let (other_tx, mut other) = mpsc::channel(8);
        let id = ctx.lock().add_client(tx);
        ctx.lock().add_client(other_tx);
        let search = |query: &str| ClientMsg::Search {
            pane,
            query: query.into(),
            from: at(1, 0),
            backward: true,
        };
        reply(&ctx, id, search("world"));
        reply(&ctx, id, search("absent"));
        let copy = ClientMsg::Copy {
            pane,
            start: at(0, 6),
            end: at(1, 2),
            shape: SelectShape::Chars,
        };
        reply(&ctx, id, copy);
        reply(&ctx, id, ClientMsg::Resync(PaneId(9)));
        let got: Vec<ServerMsg> = std::iter::from_fn(|| asker.try_recv().ok())
            .flatten()
            .collect();
        assert_eq!(
            got,
            [
                ServerMsg::SearchResult {
                    pane,
                    found: Some((at(0, 6), at(0, 10)))
                },
                ServerMsg::SearchResult { pane, found: None },
                ServerMsg::Clipboard("world\nbye".into()),
            ]
        );
        assert!(other.try_recv().is_err(), "nothing for the other client");
    }
}
