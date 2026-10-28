//! What every server task shares.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use monarch_core::PaneId;
use monarch_proto::{AgentReport, Review};
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    CliGitRunner, DiffReader, InputRouter, PaneEnv, PaneEvent, PaneIo, PaneRuntime, PaneSpec,
    PersistedPane, Placement, ProcessProbe, ServerError, SessionState, SpawnParams, SystemProbe,
    WorktreeManager,
};

mod explain;
mod input;
mod session;
mod worktrees;

/// The session state plus the wake-ups and channels around it.
pub struct ServerContext {
    state: Mutex<SessionState>,
    /// What each live pane was started with, for the state file.
    specs: Mutex<BTreeMap<PaneId, PaneSpec>>,
    /// Each pane's input, reachable without `state`.
    inputs: InputRouter,
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
    diffs: DiffReader,
    worktrees: WorktreeManager,
}

impl ServerContext {
    /// A context for an empty session.
    #[must_use]
    pub fn new(env: PaneEnv, events: UnboundedSender<PaneEvent>, scrollback: usize) -> Self {
        Self {
            state: Mutex::new(SessionState::default()),
            specs: Mutex::new(BTreeMap::new()),
            inputs: InputRouter::default(),
            wake: Arc::new(Notify::new()),
            shutdown: Notify::new(),
            persist: Notify::new(),
            events,
            env,
            scrollback,
            probe: Box::new(SystemProbe::new()),
            diffs: DiffReader::new(Box::new(CliGitRunner)),
            worktrees: WorktreeManager::new(Box::new(CliGitRunner)),
        }
    }

    /// The session's name.
    #[must_use]
    pub fn session(&self) -> &str {
        &self.env.session
    }

    /// The same context managing worktrees with `worktrees` (tests use a
    /// fake git).
    #[must_use]
    pub fn with_worktrees(mut self, worktrees: WorktreeManager) -> Self {
        self.worktrees = worktrees;
        self
    }

    /// The same context reading reviews with `diffs` (tests use a fake
    /// git).
    #[must_use]
    pub fn with_diffs(mut self, diffs: DiffReader) -> Self {
        self.diffs = diffs;
        self
    }

    /// The uncommitted changes in `pane`'s checkout, or `None` when it
    /// isn't in one. Runs git: call it off the async runtime.
    #[must_use]
    pub fn review(&self, pane: PaneId) -> Option<Review> {
        let place = self.lock().place(pane)?;
        let mut review = self.diffs.review(Path::new(place.worktree.as_deref()?));
        review.branch = place.branch;
        Some(review)
    }

    /// Opens `path` from `pane`'s checkout with `editor`, zoomed over the
    /// focused pane. Returns the new pane, or `None` when `pane` isn't in
    /// a checkout or `path` would leave it.
    ///
    /// # Errors
    /// The PTY or program could not be started.
    pub fn edit(
        &self,
        pane: PaneId,
        path: &str,
        editor: &str,
    ) -> Result<Option<PaneId>, ServerError> {
        match self.edit_spec(pane, path, editor) {
            Some(spec) => self.spawn(&spec, None, Placement::Zoomed).map(Some),
            None => Ok(None),
        }
    }

    /// What [`Self::edit`] would run, if the file is inside the checkout.
    fn edit_spec(&self, pane: PaneId, path: &str, editor: &str) -> Option<PaneSpec> {
        let inside = Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
        let root = self.lock().place(pane)?.worktree?;
        (inside && !path.is_empty() && !editor.trim().is_empty())
            .then(|| PaneSpec::edit(editor, path, PathBuf::from(root)))
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
        if let Some(input) = runtime.input() {
            self.inputs.add(id, input);
        }
        let mut state = self.lock();
        state.insert_pane_at(id, Box::new(runtime), placement);
        if let Some(agent) = &spec.agent {
            state.launched_agent(id, agent.kind);
        }
        drop(state);
        self.remember(id, spec.clone());
        self.wake();
        Ok(id)
    }

    /// Drops what was recorded for an exited pane.
    pub fn forget_pane(&self, id: PaneId) {
        self.inputs.remove(id);
        self.lock_specs().remove(&id);
        self.persist.notify_one();
    }

    /// Keeps the session id an agent pane's hooks report, so a restart
    /// resumes that session (Codex picks its own id). Panes Monarch did
    /// not launch as that agent are left alone.
    pub fn record_session(&self, pane: PaneId, report: &AgentReport) {
        let mut specs = self.lock_specs();
        let Some(agent) = specs.get_mut(&pane).and_then(|s| s.agent.as_mut()) else {
            return;
        };
        if agent.kind != report.agent {
            return;
        }
        let before = agent.clone();
        agent.history = agent.history.after(report.state);
        if let Some(id) = &report.session_id {
            agent.session_id = Some(id.clone());
        }
        if *agent != before {
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
    use monarch_proto::PanePlace;

    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    #[test]
    fn edits_stay_inside_the_checkout() {
        let ctx = context();
        let id = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        assert_eq!(ctx.edit_spec(id, "src/a.rs", "vi"), None, "no checkout");
        let place = PanePlace {
            cwd: "/repo/src".into(),
            project: Some("repo".into()),
            worktree: Some("/repo".into()),
            branch: Some("main".into()),
            status: None,
        };
        ctx.lock().set_place(id, Some(place));
        let spec = ctx.edit_spec(id, "src/a.rs", "vi").expect("inside");
        assert_eq!(spec.cwd.as_deref(), Some(std::path::Path::new("/repo")));
        for bad in ["../x", "/etc/passwd", "a/../../b", ""] {
            assert_eq!(ctx.edit_spec(id, bad, "vi"), None, "{bad}");
        }
        assert_eq!(ctx.edit_spec(id, "a.rs", " "), None, "no editor");
    }

    #[test]
    fn specs_follow_panes_in_and_out() {
        let ctx = context();
        let id = ctx.insert_for_test(PaneSpec::shell_command("top"), FakePane::shell());
        assert_eq!(ctx.specs()[&id], PaneSpec::shell_command("top"));
        ctx.forget_pane(id);
        assert!(ctx.specs().is_empty());
        ctx.forget_pane(id); // twice is harmless
    }

    #[test]
    fn hook_reports_record_the_session_and_whether_it_talked() {
        use monarch_core::{AgentKind, AgentState};
        use monarch_proto::AgentReport;

        use crate::AgentHistory;

        let ctx = context();
        let id = ctx.insert_for_test(PaneSpec::agent(AgentKind::Codex), FakePane::shell());
        let history = |ctx: &super::ServerContext| ctx.specs()[&id].agent.clone().expect("agent");
        let start = AgentReport {
            state: Some(AgentState::Idle),
            session_id: Some("s1".into()),
            ..AgentReport::new(id, AgentKind::Codex)
        };
        ctx.record_session(id, &start);
        let agent = history(&ctx);
        assert_eq!(
            (agent.history, agent.session_id.as_deref()),
            (AgentHistory::Empty, Some("s1"))
        );
        let prompt = AgentReport {
            state: Some(AgentState::Working),
            ..AgentReport::new(id, AgentKind::Codex)
        };
        ctx.record_session(id, &prompt);
        assert_eq!(history(&ctx).history, AgentHistory::Started);
        ctx.record_session(id, &AgentReport::new(id, AgentKind::Claude));
        assert_eq!(
            history(&ctx).history,
            AgentHistory::Started,
            "another agent's report"
        );
    }
}
