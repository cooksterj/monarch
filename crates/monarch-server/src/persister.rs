//! Keeps the session's state file current so a killed server can be
//! restored (spike 6; M5 persistence builds on it).

use std::sync::Arc;
use std::time::Duration;

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

    /// Saves on every pane change and every [`PERSIST_INTERVAL`].
    pub async fn run(mut self, ctx: Arc<ServerContext>) {
        loop {
            tokio::select! {
                () = tokio::time::sleep(PERSIST_INTERVAL) => {}
                () = ctx.persist.notified() => {}
            }
            if let Err(e) = self.save_if_changed(&ctx) {
                eprintln!("monarch: {e}");
            }
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
        let (panes, focus) = {
            let state = ctx.lock();
            let ids = state.pane_ids();
            let focus = ids.iter().position(|id| Some(*id) == state.focus());
            let pids: Vec<_> = ids
                .iter()
                .map(|id| (*id, state.pane(*id).and_then(|io| io.process_id())))
                .collect();
            (pids, focus)
        };
        let specs = ctx.specs();
        let panes = panes
            .into_iter()
            .filter_map(|(id, pid)| {
                let spec = specs.get(&id)?;
                Some(PersistedPane::new(
                    spec,
                    pid.and_then(|p| self.probe.cwd(p)),
                ))
            })
            .collect();
        PersistedSession::new(panes, focus)
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
        };
        ctx.insert_for_test(spec, FakePane::shell());
        let session = persister.capture(&ctx);
        assert_eq!(session.panes[0].cwd, Some(PathBuf::from("/start")));
    }
}
