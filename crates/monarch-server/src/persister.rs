//! Keeps the session's state file current so a killed server can be
//! restored (spike 6; M5 persistence builds on it).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use crate::{PersistedPane, PersistedSession, ProcessProbe, ServerContext, StateFile};

/// How often working directories are re-read. A `kill -9` loses at most
/// this much, and a `cd` is the only change that isn't announced.
pub const PERSIST_INTERVAL: Duration = Duration::from_secs(2);

/// Writes the state file whenever what it would say changes.
pub struct Persister {
    file: StateFile,
    probe: Box<dyn ProcessProbe>,
    last: Option<PersistedSession>,
}

impl Persister {
    /// A persister writing `file`, reading directories with `probe`.
    #[must_use]
    pub fn new(file: StateFile, probe: Box<dyn ProcessProbe>) -> Self {
        Self {
            file,
            probe,
            last: None,
        }
    }

    /// Saves on every pane change and every [`PERSIST_INTERVAL`] until
    /// `stop` is notified. A save under way finishes first, so once this
    /// returns nothing writes the file: the server awaits it before
    /// deleting the file (aborting the task would leave a blocking save
    /// running, which could write the file back).
    pub async fn run(mut self, ctx: Arc<ServerContext>, stop: Arc<Notify>) {
        loop {
            tokio::select! {
                biased;
                () = stop.notified() => return,
                () = tokio::time::sleep(PERSIST_INTERVAL) => {}
                () = ctx.persist.notified() => {}
            }
            // Off the async threads: it asks the OS for directories and
            // writes a file.
            let shared = Arc::clone(&ctx);
            self = match tokio::task::spawn_blocking(move || {
                if let Err(e) = self.save_if_changed(&shared) {
                    eprintln!("monarch: {e}");
                }
                self
            })
            .await
            {
                Ok(persister) => persister,
                Err(_) => return,
            };
        }
    }

    /// Writes the file if the session changed since the last write.
    /// Returns whether it wrote.
    ///
    /// # Errors
    /// The file could not be written.
    pub fn save_if_changed(&mut self, ctx: &ServerContext) -> Result<bool, crate::ServerError> {
        let session = self.capture(ctx);
        if !session.is_restorable() || self.last.as_ref() == Some(&session) {
            return Ok(false);
        }
        self.file.save(&session)?;
        self.last = Some(session);
        Ok(true)
    }

    /// The session as it would be restored now. Directories are read
    /// without holding the session lock.
    #[must_use]
    pub fn capture(&self, ctx: &ServerContext) -> PersistedSession {
        let (panes, focus, tabs, active) = {
            let state = ctx.lock();
            let ids = state.pane_ids();
            let focus = ids.iter().position(|id| Some(*id) == state.focus());
            let pids: Vec<_> = ids
                .iter()
                .map(|id| (*id, state.pane(*id).and_then(|io| io.process_id())))
                .collect();
            (pids, focus, state.tabs().to_vec(), state.active_tab())
        };
        let specs = ctx.specs();
        let panes = panes
            .into_iter()
            .filter_map(|(id, pid)| {
                let spec = specs.get(&id)?;
                let cwd = pid.and_then(|p| self.probe.cwd(p));
                Some(PersistedPane {
                    id: Some(id),
                    ..PersistedPane::new(spec, cwd)
                })
            })
            .collect();
        PersistedSession::new(panes, focus).with_tabs(tabs, active)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::test_support::{FakePane, context};
    use crate::{PaneSpec, ProcessProbe};

    struct EveryoneIn(PathBuf);

    impl ProcessProbe for EveryoneIn {
        fn cwd(&self, _pid: u32) -> Option<PathBuf> {
            Some(self.0.clone())
        }
    }

    #[test]
    fn saves_panes_with_live_cwd_only_when_something_changed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = StateFile::new(dir.path().join("state.json"));
        let mut persister = Persister::new(file.clone(), Box::new(EveryoneIn("/live".into())));
        let ctx = context();
        assert!(!persister.save_if_changed(&ctx).expect("save"), "no panes");

        ctx.insert_for_test(PaneSpec::shell_command("htop"), FakePane::with_pid(42));
        assert!(persister.save_if_changed(&ctx).expect("save"));
        assert!(!persister.save_if_changed(&ctx).expect("save"), "unchanged");

        let saved = file.load().expect("load").expect("saved");
        assert_eq!(saved.focus, Some(0));
        assert_eq!(saved.panes[0].cwd, Some(PathBuf::from("/live")));
        assert_eq!(saved.panes[0].argv, PaneSpec::shell_command("htop").argv);
    }

    #[tokio::test]
    async fn once_stopped_nothing_writes_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = StateFile::new(dir.path().join("state.json"));
        let ctx = Arc::new(context());
        ctx.insert_for_test(PaneSpec::shell_command("htop"), FakePane::with_pid(42));
        let persister = Persister::new(file.clone(), Box::new(EveryoneIn("/live".into())));
        let stop = Arc::new(Notify::new());
        let task = tokio::spawn(persister.run(Arc::clone(&ctx), Arc::clone(&stop)));
        ctx.persist.notify_one();
        while file.load().ok().flatten().is_none() {
            tokio::task::yield_now().await;
        }
        stop.notify_one();
        ctx.persist.notify_one();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("stops")
            .expect("joins");
        file.remove();
        ctx.insert_for_test(PaneSpec::shell_command("top"), FakePane::with_pid(43));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            file.load().expect("load").is_none(),
            "removed stays removed"
        );
    }

    #[test]
    fn a_pane_without_a_process_keeps_its_start_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let persister = Persister::new(
            StateFile::new(dir.path().join("state.json")),
            Box::new(EveryoneIn("/live".into())),
        );
        let ctx = context();
        let spec = PaneSpec {
            argv: Vec::new(),
            cwd: Some("/start".into()),
            agent: None,
        };
        ctx.insert_for_test(spec, FakePane::shell());
        let session = persister.capture(&ctx);
        assert_eq!(session.panes[0].cwd, Some(PathBuf::from("/start")));
    }
}
