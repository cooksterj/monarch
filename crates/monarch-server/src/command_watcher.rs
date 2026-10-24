//! Names panes after their foreground program (`zsh`, `vim`), the way
//! tmux names windows, for programs that set no title.

use std::sync::Arc;
use std::time::Duration;

use crate::{ProcessProbe, ServerContext};

/// How often foreground programs are re-read. Starting or leaving a
/// program is never announced, so this is the most a label lags.
pub const COMMAND_INTERVAL: Duration = Duration::from_millis(500);

/// Polls each pane's foreground process and publishes its name.
pub struct CommandWatcher {
    probe: Box<dyn ProcessProbe>,
}

impl CommandWatcher {
    /// A watcher reading names with `probe`.
    #[must_use]
    pub fn new(probe: Box<dyn ProcessProbe>) -> Self {
        Self { probe }
    }

    /// Checks every [`COMMAND_INTERVAL`] until aborted.
    pub async fn run(self, ctx: Arc<ServerContext>) {
        loop {
            self.check(&ctx);
            tokio::time::sleep(COMMAND_INTERVAL).await;
        }
    }

    /// Reads every pane's foreground program once, without holding the
    /// session lock while asking the OS; returns how many changed.
    pub fn check(&self, ctx: &ServerContext) -> usize {
        let pids = ctx.lock().foreground_processes();
        let names: Vec<_> = pids
            .into_iter()
            .map(|(pane, pid)| (pane, self.probe.name(pid)))
            .collect();
        let mut state = ctx.lock();
        names
            .into_iter()
            .filter(|(pane, name)| state.set_command(*pane, name.clone()))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use monarch_proto::ServerMsg;
    use tokio::sync::mpsc;

    use super::*;
    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    /// Names every process after whatever it was last told.
    struct Named(Arc<Mutex<Option<String>>>);

    impl ProcessProbe for Named {
        fn cwd(&self, _pid: u32) -> Option<PathBuf> {
            None
        }
        fn name(&self, _pid: u32) -> Option<String> {
            self.0.lock().expect("lock").clone()
        }
    }

    #[test]
    fn publishes_a_name_only_when_it_changes() {
        let name = Arc::new(Mutex::new(Some("zsh".into())));
        let ctx = context();
        let (tx, mut rx) = mpsc::channel(8);
        ctx.lock().add_client(tx);
        let pane = ctx.insert_for_test(PaneSpec::shell_command("zsh"), FakePane::with_pid(7));
        ctx.insert_for_test(PaneSpec::shell_command("x"), FakePane::shell());
        let watcher = CommandWatcher::new(Box::new(Named(Arc::clone(&name))));
        assert_eq!(
            watcher.check(&ctx),
            1,
            "the pane without a process is skipped"
        );
        assert_eq!(watcher.check(&ctx), 0, "unchanged");
        *name.lock().expect("lock") = Some("vim".into());
        assert_eq!(watcher.check(&ctx), 1);
        let sent: Vec<ServerMsg> = std::iter::from_fn(|| rx.try_recv().ok())
            .flatten()
            .collect();
        assert_eq!(
            sent,
            [
                ServerMsg::Command {
                    pane,
                    command: Some("zsh".into())
                },
                ServerMsg::Command {
                    pane,
                    command: Some("vim".into())
                }
            ]
        );
        let tabs = ctx.lock().layout().tabs;
        assert_eq!(tabs[0].panes[0].command.as_deref(), Some("vim"));
    }
}
