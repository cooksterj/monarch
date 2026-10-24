//! Handles pane events: titles, bells, stalled sync frames and exits.

use std::sync::Arc;
use std::time::Instant;

use monarch_core::PaneId;
use monarch_proto::ServerMsg;
use monarch_term::EngineEvent;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{PaneEvent, ServerContext};

/// Larger copies are dropped: no clipboard needs them, and they would
/// crowd every client's render lane.
pub const MAX_CLIPBOARD: usize = 1 << 20;

/// The task that turns [`PaneEvent`]s into session changes and messages.
#[derive(Debug, Clone, Copy, Default)]
pub struct EventLoop;

impl EventLoop {
    /// Runs until every pane thread has gone away.
    pub async fn run(ctx: Arc<ServerContext>, mut events: UnboundedReceiver<PaneEvent>) {
        while let Some(event) = events.recv().await {
            Self::handle(&ctx, event);
        }
    }

    fn handle(ctx: &Arc<ServerContext>, event: PaneEvent) {
        match event {
            PaneEvent::Title { pane, title } => ctx.lock().set_title(pane, title),
            PaneEvent::Bell(pane) => {
                let mut state = ctx.lock();
                if state.visible_panes().contains(&pane) {
                    state.broadcast(&ServerMsg::HostWrite(vec![0x07]));
                }
            }
            PaneEvent::Clipboard { pane, text } => {
                // Only a pane the user can see may set their clipboard, so
                // a background program cannot overwrite it unnoticed.
                let mut state = ctx.lock();
                if text.len() <= MAX_CLIPBOARD && state.visible_panes().contains(&pane) {
                    state.broadcast(&ServerMsg::Clipboard(text));
                }
            }
            PaneEvent::SyncPending { pane, deadline } => {
                let ctx = Arc::clone(ctx);
                tokio::spawn(async move {
                    tokio::time::sleep_until(deadline.into()).await;
                    force_sync(&ctx, pane);
                });
            }
            PaneEvent::Exited { pane, .. } => {
                let empty = {
                    let mut state = ctx.lock();
                    state.broadcast(&ServerMsg::PaneExited { pane });
                    state.remove_pane(pane)
                };
                ctx.wake();
                if empty {
                    ctx.shutdown.notify_one();
                }
            }
        }
    }
}

/// Ends a synchronized update the program never finished (it died or
/// stalled mid-frame), so its pane cannot freeze.
fn force_sync(ctx: &ServerContext, pane: PaneId) {
    let state = ctx.lock();
    let Some(io) = state.pane(pane) else {
        return;
    };
    let mut events = Vec::new();
    {
        let mut term = io.shared().lock();
        if term.sync_deadline().is_some_and(|d| d <= Instant::now()) {
            term.end_sync(&mut events);
        }
    }
    for event in events {
        if let EngineEvent::Reply(bytes) = event {
            io.send(bytes);
        }
    }
    if io.shared().mark_dirty() {
        ctx.wake();
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::ServerMsg;
    use tokio::sync::mpsc;

    use super::*;
    use crate::test_support::FakePane;
    use crate::{DEFAULT_SIZE, PaneEnv};

    fn context() -> Arc<ServerContext> {
        let (events, _rx) = mpsc::unbounded_channel();
        let env = PaneEnv {
            session: "t".into(),
            socket: "/tmp/t.sock".into(),
        };
        let ctx = Arc::new(ServerContext::new(env, events, 10));
        let id = ctx.lock().allocate_pane_id();
        ctx.lock().insert_pane(id, FakePane::boxed(DEFAULT_SIZE));
        ctx
    }

    #[tokio::test]
    async fn last_pane_exiting_shuts_the_server_down() {
        let ctx = context();
        let (lane, mut rx) = mpsc::channel(8);
        ctx.lock().add_client(lane);
        EventLoop::handle(
            &ctx,
            PaneEvent::Exited {
                pane: PaneId(1),
                code: Some(0),
            },
        );
        assert!(ctx.lock().pane_ids().is_empty());
        assert_eq!(
            rx.try_recv().expect("exit notice"),
            [ServerMsg::PaneExited { pane: PaneId(1) }]
        );
        tokio::time::timeout(std::time::Duration::from_secs(1), ctx.shutdown.notified())
            .await
            .expect("shutdown signalled");
    }

    #[tokio::test]
    async fn titles_and_bells_reach_clients() {
        let ctx = context();
        let (lane, mut rx) = mpsc::channel(8);
        ctx.lock().add_client(lane);
        EventLoop::handle(
            &ctx,
            PaneEvent::Title {
                pane: PaneId(1),
                title: Some("vim".into()),
            },
        );
        EventLoop::handle(&ctx, PaneEvent::Bell(PaneId(1)));
        EventLoop::handle(&ctx, PaneEvent::Bell(PaneId(9)));
        let got: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok())
            .flatten()
            .collect();
        assert_eq!(
            got,
            [
                ServerMsg::Title {
                    pane: PaneId(1),
                    title: Some("vim".into())
                },
                ServerMsg::HostWrite(vec![0x07]),
            ]
        );
    }

    #[tokio::test]
    async fn only_visible_panes_may_set_the_clipboard() {
        let ctx = context();
        let (lane, mut rx) = mpsc::channel(8);
        ctx.lock().add_client(lane);
        let copy = |pane, text: &str| PaneEvent::Clipboard {
            pane: PaneId(pane),
            text: text.into(),
        };
        EventLoop::handle(&ctx, copy(1, "visible"));
        EventLoop::handle(&ctx, copy(9, "hidden"));
        EventLoop::handle(&ctx, copy(1, &"x".repeat(MAX_CLIPBOARD + 1)));
        let got: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok())
            .flatten()
            .collect();
        assert_eq!(got, [ServerMsg::Clipboard("visible".into())]);
    }

    #[tokio::test]
    async fn a_stalled_sync_frame_is_forced_out() {
        let ctx = context();
        {
            let state = ctx.lock();
            let io = state.pane(PaneId(1)).expect("pane");
            io.shared()
                .lock()
                .feed(b"\x1b[?2026hhidden", &mut Vec::new());
            io.shared().take_dirty();
        }
        let deadline = ctx
            .lock()
            .pane(PaneId(1))
            .and_then(|io| io.shared().lock().sync_deadline());
        tokio::time::sleep_until(deadline.expect("sync pending").into()).await;
        force_sync(&ctx, PaneId(1));
        let state = ctx.lock();
        let io = state.pane(PaneId(1)).expect("pane");
        assert!(io.shared().lock().text().starts_with("hidden"));
        assert!(io.shared().take_dirty(), "pane redrawn after forcing");
    }
}
