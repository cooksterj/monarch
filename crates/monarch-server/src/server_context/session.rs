//! The session as a whole: what it holds, and ending it on purpose.

use monarch_proto::SessionInfo;

use super::ServerContext;

impl ServerContext {
    /// Panes, tabs and attached clients, for `monarch ls`.
    #[must_use]
    pub fn info(&self) -> SessionInfo {
        let state = self.lock();
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        SessionInfo {
            panes: count(state.pane_ids().len()),
            tabs: count(state.tabs().len()),
            clients: count(state.client_count()),
            pid: std::process::id(),
        }
    }

    /// Hangs up every pane and stops the server, which then removes its
    /// state file: a session ended on purpose is not restored.
    pub fn kill(&self) {
        {
            let state = self.lock();
            for id in state.pane_ids() {
                if let Some(io) = state.pane(id) {
                    io.close();
                }
            }
        }
        self.shutdown.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use futures_util::FutureExt;

    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    #[test]
    fn info_counts_and_kill_hangs_up_and_stops() {
        let ctx = context();
        let (io, closed) = FakePane::tracked();
        ctx.insert_for_test(PaneSpec::default(), io);
        ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        let info = ctx.info();
        assert_eq!((info.panes, info.tabs, info.clients), (2, 2, 0));
        assert_eq!(info.pid, std::process::id());
        assert!(ctx.shutdown.notified().now_or_never().is_none(), "running");
        ctx.kill();
        assert!(closed.load(Ordering::SeqCst), "hung up");
        assert!(ctx.shutdown.notified().now_or_never().is_some(), "stopping");
    }
}
