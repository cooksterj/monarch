//! Detects what agents without hooks are doing by reading their panes'
//! screens with the agent's detection manifest (chosen by foreground
//! program), a few times a second and only after new output.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use monarch_core::{AgentState, PaneId};
use monarch_detect::{DetectionInput, ManifestRegistry};

use crate::ServerContext;

/// How often screens with new output are read (at most 4 Hz).
pub const SCREEN_INTERVAL: Duration = Duration::from_millis(250);

/// How long a screen must keep reading idle before it counts: agents
/// pause between the steps of one turn.
pub const IDLE_SETTLE: Duration = Duration::from_secs(1);

/// One agent pane's screen, as read for detection.
pub(crate) struct PaneScreen {
    pub pane: PaneId,
    /// The foreground program, which picks the manifest.
    pub program: String,
    /// The window title the program set.
    pub title: String,
    /// The live screen's text; `None` if unchanged since the last read.
    pub text: Option<String>,
}

/// Reads agent screens and applies their manifests' verdicts.
pub struct ScreenWatcher {
    registry: ManifestRegistry,
    /// Panes whose program has a manifest, as of the last check.
    watched: BTreeSet<PaneId>,
    /// Panes whose screen reads idle: since when, or `None` once applied.
    idle: BTreeMap<PaneId, Option<Instant>>,
}

impl ScreenWatcher {
    /// A watcher using `registry`'s manifests.
    #[must_use]
    pub fn new(registry: ManifestRegistry) -> Self {
        Self {
            registry,
            watched: BTreeSet::new(),
            idle: BTreeMap::new(),
        }
    }

    /// Checks every [`SCREEN_INTERVAL`] until aborted, off the async
    /// threads (manifests are CPU work that would hold up keystrokes).
    pub async fn run(mut self, ctx: Arc<ServerContext>) {
        loop {
            let shared = Arc::clone(&ctx);
            self = match tokio::task::spawn_blocking(move || {
                self.check(&shared, Instant::now());
                self
            })
            .await
            {
                Ok(watcher) => watcher,
                Err(_) => return,
            };
            tokio::time::sleep(SCREEN_INTERVAL).await;
        }
    }

    /// Reads the screens that changed and applies what they show; the
    /// manifests run without the session lock held.
    pub fn check(&mut self, ctx: &ServerContext, now: Instant) {
        let registry = &self.registry;
        let screens = ctx
            .lock()
            .screens(|p| registry.for_program(p).is_some(), &self.watched);
        let current: BTreeSet<PaneId> = screens.iter().map(|s| s.pane).collect();
        let gone: Vec<PaneId> = self.watched.difference(&current).copied().collect();
        self.idle.retain(|pane, _| current.contains(pane));
        self.watched = current;
        let seen: Vec<_> = screens
            .iter()
            .filter_map(|s| Some((s.pane, self.verdict(s, now)?)))
            .collect();
        let mut state = ctx.lock();
        gone.into_iter().for_each(|pane| state.screen_gone(pane));
        for (pane, agent_state) in seen {
            state.screen_seen(pane, agent_state);
        }
    }

    /// The state to apply to one pane now, if any: what its screen
    /// shows, with idle held back until it has lasted [`IDLE_SETTLE`].
    fn verdict(&mut self, screen: &PaneScreen, now: Instant) -> Option<AgentState> {
        if let Some(text) = &screen.text {
            let manifest = self.registry.for_program(&screen.program)?;
            let input = DetectionInput {
                screen: text,
                osc_title: &screen.title,
                osc_progress: "",
            };
            match manifest.evaluate(input).state() {
                Some(AgentState::Idle) => {
                    self.idle.entry(screen.pane).or_insert(Some(now));
                }
                other => {
                    self.idle.remove(&screen.pane);
                    return other;
                }
            }
        }
        let pending = self.idle.get_mut(&screen.pane)?;
        pending.filter(|since| now.duration_since(*since) >= IDLE_SETTLE)?;
        *pending = None;
        Some(AgentState::Idle)
    }
}

#[cfg(test)]
mod tests;
