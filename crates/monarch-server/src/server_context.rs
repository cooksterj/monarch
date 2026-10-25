//! What every server task shares.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use monarch_core::{AgentState, PaneId};
use monarch_proto::AgentReport;
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    PaneEnv, PaneEvent, PaneRuntime, PaneSpec, PersistedPane, Placement, ProcessProbe, ServerError,
    SessionState, SpawnParams, SystemProbe,
};

/// The session state plus the wake-ups and channels around it.
pub struct ServerContext {
    state: Mutex<SessionState>,
    /// What each live pane was started with, for the state file.
    specs: Mutex<BTreeMap<PaneId, PaneSpec>>,
    /// Wakes the frame coalescer (new output, attach, resize, focus).
    pub wake: Arc<Notify>,
    /// Signalled when the last pane exits.
    pub shutdown: Notify,
    /// Wakes the persister (a pane started or exited).
    pub persist: Notify,
    events: UnboundedSender<PaneEvent>,
    env: PaneEnv,
    scrollback: usize,
    probe: Box<dyn ProcessProbe>,
}

impl ServerContext {
    /// A context for an empty session.
    #[must_use]
    pub fn new(env: PaneEnv, events: UnboundedSender<PaneEvent>, scrollback: usize) -> Self {
        Self {
            state: Mutex::new(SessionState::default()),
            specs: Mutex::new(BTreeMap::new()),
            wake: Arc::new(Notify::new()),
            shutdown: Notify::new(),
            persist: Notify::new(),
            events,
            env,
            scrollback,
            probe: Box::new(SystemProbe::new()),
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

    /// Starts a pane running `spec` in a background tab.
    ///
    /// # Errors
    /// The PTY or program could not be started.
    pub fn spawn_pane(&self, spec: &PaneSpec) -> Result<PaneId, ServerError> {
        self.spawn(spec, None, Placement::BACKGROUND)
    }

    /// Starts a shell where `placement` says, in the focused pane's
    /// current directory (as a new terminal tab would).
    ///
    /// # Errors
    /// The PTY or program could not be started.
    pub fn spawn_shell_at(&self, placement: Placement) -> Result<PaneId, ServerError> {
        let spec = PaneSpec {
            cwd: self.focused_cwd(),
            ..PaneSpec::default()
        };
        self.spawn(&spec, None, placement)
    }

    /// The focused pane's working directory, if it can be read.
    #[must_use]
    pub fn focused_cwd(&self) -> Option<PathBuf> {
        let pid = {
            let state = self.lock();
            state.focus().and_then(|p| state.pane(p)?.process_id())
        }?;
        self.probe.cwd(pid)
    }

    /// Starts the pane that replaces `pane` after a restart, with a notice
    /// above its first output.
    ///
    /// # Errors
    /// The PTY or program could not be started.
    pub fn spawn_restored(&self, pane: &PersistedPane) -> Result<PaneId, ServerError> {
        self.spawn(&pane.restore(), Some(&pane.banner()), Placement::BACKGROUND)
    }

    fn spawn(
        &self,
        spec: &PaneSpec,
        banner: Option<&str>,
        placement: Placement,
    ) -> Result<PaneId, ServerError> {
        let (id, size, profile) = {
            let mut state = self.lock();
            (state.allocate_pane_id(), state.size(), state.host_profile())
        };
        let runtime = PaneRuntime::spawn(SpawnParams {
            id,
            spec,
            banner,
            env: &self.env,
            size,
            profile,
            scrollback: self.scrollback,
            events: self.events.clone(),
            wake: Arc::clone(&self.wake),
        })?;
        let mut state = self.lock();
        state.insert_pane_at(id, Box::new(runtime), placement);
        if let Some(agent) = &spec.agent {
            state.report_agent(&AgentReport {
                state: Some(AgentState::Idle),
                ..AgentReport::new(id, agent.kind)
            });
        }
        drop(state);
        self.remember(id, spec.clone());
        self.wake();
        Ok(id)
    }

    /// Drops what was recorded for an exited pane.
    pub fn forget_pane(&self, id: PaneId) {
        self.lock_specs().remove(&id);
        self.persist.notify_one();
    }

    /// Keeps the session id an agent pane's hooks report, so a restart
    /// resumes that session (Codex picks its own id). Panes Monarch did
    /// not launch as that agent are left alone.
    pub fn record_session(&self, pane: PaneId, report: &AgentReport) {
        let Some(id) = &report.session_id else {
            return;
        };
        let mut specs = self.lock_specs();
        let Some(agent) = specs.get_mut(&pane).and_then(|s| s.agent.as_mut()) else {
            return;
        };
        if agent.kind == report.agent && agent.session_id.as_ref() != Some(id) {
            agent.session_id = Some(id.clone());
            drop(specs);
            self.persist.notify_one();
        }
    }

    /// What each live pane was started with.
    #[must_use]
    pub fn specs(&self) -> BTreeMap<PaneId, PaneSpec> {
        self.lock_specs().clone()
    }

    fn remember(&self, id: PaneId, spec: PaneSpec) {
        self.lock_specs().insert(id, spec);
        self.persist.notify_one();
    }

    fn lock_specs(&self) -> MutexGuard<'_, BTreeMap<PaneId, PaneSpec>> {
        self.specs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Adds a pane without a PTY, for tests of the layers above.
    #[cfg(test)]
    pub(crate) fn insert_for_test(&self, spec: PaneSpec, io: Box<dyn crate::PaneIo>) -> PaneId {
        let id = self.lock().allocate_pane_id();
        self.lock().insert_pane(id, io);
        self.remember(id, spec);
        id
    }
}

#[cfg(test)]
mod tests {
    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    #[test]
    fn specs_follow_panes_in_and_out() {
        let ctx = context();
        let id = ctx.insert_for_test(PaneSpec::shell_command("top"), FakePane::shell());
        assert_eq!(ctx.specs()[&id], PaneSpec::shell_command("top"));
        ctx.forget_pane(id);
        assert!(ctx.specs().is_empty());
        ctx.forget_pane(id); // twice is harmless
    }
}
