//! What every server task shares.

use std::sync::{Arc, Mutex, MutexGuard};

use monarch_core::PaneId;
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedSender;

use crate::{PaneEnv, PaneEvent, PaneRuntime, PaneSpec, ServerError, SessionState, SpawnParams};

/// The session state plus the wake-ups and channels around it.
pub struct ServerContext {
    state: Mutex<SessionState>,
    /// Wakes the frame coalescer (new output, attach, resize, focus).
    pub wake: Arc<Notify>,
    /// Signalled when the last pane exits.
    pub shutdown: Notify,
    events: UnboundedSender<PaneEvent>,
    env: PaneEnv,
    scrollback: usize,
}

impl ServerContext {
    /// A context for an empty session.
    #[must_use]
    pub fn new(env: PaneEnv, events: UnboundedSender<PaneEvent>, scrollback: usize) -> Self {
        Self {
            state: Mutex::new(SessionState::default()),
            wake: Arc::new(Notify::new()),
            shutdown: Notify::new(),
            events,
            env,
            scrollback,
        }
    }

    /// Locks the session. Never hold this across an `.await`.
    pub fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Wakes the frame coalescer.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Starts a pane running `spec` at the session's size.
    ///
    /// # Errors
    /// The PTY or program could not be started.
    pub fn spawn_pane(&self, spec: &PaneSpec) -> Result<PaneId, ServerError> {
        let (id, size, profile) = {
            let mut state = self.lock();
            (state.allocate_pane_id(), state.size(), state.host_profile())
        };
        let runtime = PaneRuntime::spawn(SpawnParams {
            id,
            spec,
            env: &self.env,
            size,
            profile,
            scrollback: self.scrollback,
            events: self.events.clone(),
            wake: Arc::clone(&self.wake),
        })?;
        self.lock().insert_pane(id, Box::new(runtime));
        self.wake();
        Ok(id)
    }
}
