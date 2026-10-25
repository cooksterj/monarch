//! Follows each pane's working directory and the git checkout around it,
//! so the sidebar can group panes by project and worktree.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use monarch_core::PaneId;
use monarch_proto::PanePlace;

use crate::{ProcessProbe, RepoProbe, ServerContext};

/// How often directories are re-read; a `cd` shows within this.
pub const PLACE_INTERVAL: Duration = Duration::from_secs(1);

/// Every this many checks git is asked again even if no directory
/// changed, so a `git switch` shows up.
const REFRESH_EVERY: u32 = 5;

/// Polls each pane's foreground directory; asks git only when it changes
/// (or every [`REFRESH_EVERY`] checks) and publishes the pane's place.
pub struct PlaceWatcher {
    probe: Box<dyn ProcessProbe>,
    repos: RepoProbe,
    seen: BTreeMap<PaneId, PathBuf>,
    checks: u32,
}

impl PlaceWatcher {
    /// A watcher reading directories with `probe` and checkouts with
    /// `repos`.
    #[must_use]
    pub fn new(probe: Box<dyn ProcessProbe>, repos: RepoProbe) -> Self {
        Self {
            probe,
            repos,
            seen: BTreeMap::new(),
            checks: 0,
        }
    }

    /// Checks every [`PLACE_INTERVAL`] until aborted, off the async
    /// threads (git runs as a child process).
    pub async fn run(mut self, ctx: Arc<ServerContext>) {
        loop {
            let shared = Arc::clone(&ctx);
            self = match tokio::task::spawn_blocking(move || {
                self.check(&shared);
                self
            })
            .await
            {
                Ok(watcher) => watcher,
                Err(_) => return,
            };
            tokio::time::sleep(PLACE_INTERVAL).await;
        }
    }

    /// Reads every pane's directory once, without holding the session
    /// lock while asking the OS or git; returns how many places changed.
    pub fn check(&mut self, ctx: &ServerContext) -> usize {
        let refresh = self.checks.is_multiple_of(REFRESH_EVERY);
        self.checks = self.checks.wrapping_add(1);
        let pids = ctx.lock().foreground_processes();
        self.seen
            .retain(|pane, _| pids.iter().any(|(p, _)| p == pane));
        let mut asked: BTreeMap<PathBuf, PanePlace> = BTreeMap::new();
        let mut places = Vec::new();
        for (pane, pid) in pids {
            let Some(cwd) = self.probe.cwd(pid) else {
                continue;
            };
            if !refresh && self.seen.get(&pane) == Some(&cwd) {
                continue;
            }
            let place = asked
                .entry(cwd.clone())
                .or_insert_with(|| self.repos.place(&cwd))
                .clone();
            self.seen.insert(pane, cwd);
            places.push((pane, place));
        }
        let mut state = ctx.lock();
        places
            .into_iter()
            .filter(|(pane, place)| state.set_place(*pane, Some(place.clone())))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use monarch_proto::ServerMsg;
    use tokio::sync::mpsc;

    use super::*;
    use crate::test_support::{FakePane, context};
    use crate::{GitRunner, PaneSpec};

    /// Every process works in whatever directory it was last told.
    struct InDir(Arc<Mutex<PathBuf>>);

    impl ProcessProbe for InDir {
        fn cwd(&self, _pid: u32) -> Option<PathBuf> {
            Some(self.0.lock().expect("lock").clone())
        }
    }

    /// `/w/<repo>/...` is a repository on `main`; counts git runs.
    struct Repos(Arc<AtomicUsize>);

    impl GitRunner for Repos {
        fn run(&self, dir: &Path, args: &[&str]) -> Option<String> {
            self.0.fetch_add(1, Ordering::Relaxed);
            let repo = dir.strip_prefix("/w").ok()?.components().next()?;
            let top = Path::new("/w").join(repo);
            match args[0] {
                "rev-parse" => Some(format!("{}\n{}/.git", top.display(), top.display())),
                _ => Some("# branch.head main".into()),
            }
        }
    }

    /// A context with one client, a pane with a process and one without.
    fn session() -> (ServerContext, PaneId, mpsc::Receiver<Vec<ServerMsg>>) {
        let ctx = context();
        let (tx, rx) = mpsc::channel(8);
        ctx.lock().add_client(tx);
        let pane = ctx.insert_for_test(PaneSpec::shell_command("zsh"), FakePane::with_pid(7));
        ctx.insert_for_test(PaneSpec::shell_command("x"), FakePane::shell());
        (ctx, pane, rx)
    }

    #[test]
    fn publishes_places_when_the_directory_changes() {
        let dir = Arc::new(Mutex::new(PathBuf::from("/w/monarch/src")));
        let runs = Arc::new(AtomicUsize::new(0));
        let (ctx, pane, mut rx) = session();
        let repos = RepoProbe::new(Box::new(Repos(Arc::clone(&runs))));
        let mut watcher = PlaceWatcher::new(Box::new(InDir(Arc::clone(&dir))), repos);
        assert_eq!(
            watcher.check(&ctx),
            1,
            "the pane without a process is skipped"
        );
        let asked = runs.load(Ordering::Relaxed);
        assert_eq!(watcher.check(&ctx), 0);
        assert_eq!(
            runs.load(Ordering::Relaxed),
            asked,
            "same directory: git not asked"
        );
        *dir.lock().expect("lock") = PathBuf::from("/tmp");
        assert_eq!(watcher.check(&ctx), 1);
        let sent: Vec<ServerMsg> = std::iter::from_fn(|| rx.try_recv().ok())
            .flatten()
            .collect();
        let ServerMsg::Place {
            place: Some(first), ..
        } = &sent[0]
        else {
            panic!("expected a place: {sent:?}");
        };
        assert_eq!(first.project.as_deref(), Some("monarch"));
        assert_eq!(first.branch.as_deref(), Some("main"));
        let outside = Some(PanePlace::outside("/tmp"));
        assert_eq!(
            sent[1],
            ServerMsg::Place {
                pane,
                place: outside.clone()
            }
        );
        assert_eq!(ctx.lock().layout().tabs[0].panes[0].place, outside);
    }

    #[test]
    fn git_is_asked_again_now_and_then() {
        let dir = Arc::new(Mutex::new(PathBuf::from("/w/api")));
        let runs = Arc::new(AtomicUsize::new(0));
        let ctx = context();
        ctx.insert_for_test(PaneSpec::shell_command("zsh"), FakePane::with_pid(7));
        let repos = RepoProbe::new(Box::new(Repos(Arc::clone(&runs))));
        let mut watcher = PlaceWatcher::new(Box::new(InDir(dir)), repos);
        for _ in 0..REFRESH_EVERY {
            watcher.check(&ctx);
        }
        let once = runs.load(Ordering::Relaxed);
        watcher.check(&ctx);
        assert_eq!(
            runs.load(Ordering::Relaxed),
            once * 2,
            "refreshed after a while"
        );
    }
}
