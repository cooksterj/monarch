//! Paces rendering: at most one frame per budget, only when woken.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::{Instant, sleep_until};

use crate::ServerContext;

/// Default frame budget (about 60 fps).
pub const FRAME_BUDGET: Duration = Duration::from_millis(16);

/// Renders when woken: immediately if the last frame was at least one
/// budget ago, otherwise at `last + budget`. Bursts of output therefore
/// coalesce into one frame, while a keystroke's echo after idle is drawn
/// at once.
#[derive(Debug, Clone, Copy)]
pub struct FrameCoalescer {
    budget: Duration,
}

impl Default for FrameCoalescer {
    fn default() -> Self {
        Self::new(FRAME_BUDGET)
    }
}

impl FrameCoalescer {
    /// A coalescer with the given frame budget.
    #[must_use]
    pub fn new(budget: Duration) -> Self {
        Self { budget }
    }

    /// When the next frame may be drawn, given the previous one.
    #[must_use]
    pub fn next_frame(&self, last: Instant, now: Instant) -> Instant {
        (last + self.budget).max(now)
    }

    /// Runs until the task is aborted.
    pub async fn run(self, ctx: Arc<ServerContext>) {
        let mut last = Instant::now() - self.budget;
        loop {
            ctx.wake.notified().await;
            sleep_until(self.next_frame(last, Instant::now())).await;
            // Rendering is CPU work under the session lock; on a worker it
            // would hold up the tasks queued behind it there, among them
            // the connection carrying the user's next key.
            let render = Arc::clone(&ctx);
            let _ = tokio::task::spawn_blocking(move || render.lock().render_frame()).await;
            last = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_at_once_after_idle_and_paces_bursts() {
        let c = FrameCoalescer::new(Duration::from_millis(16));
        let now = Instant::now();
        let idle = now - Duration::from_secs(1);
        assert_eq!(c.next_frame(idle, now), now);
        let recent = now - Duration::from_millis(4);
        assert_eq!(
            c.next_frame(recent, now),
            recent + Duration::from_millis(16)
        );
    }
}
