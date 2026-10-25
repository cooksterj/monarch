//! `agent.explain`: each pane's status, where it came from, and what its
//! agent's detection manifest makes of the screen now.

use monarch_core::PaneId;
use monarch_detect::{DetectionInput, Manifest, ManifestRegistry};
use monarch_proto::{PaneExplanation, PaneSummary, RuleOutcome};

use super::ServerContext;

/// A pane as read under the session lock, before the manifest runs.
struct Snapshot {
    explained: PaneExplanation,
    title: String,
    text: String,
}

impl ServerContext {
    /// Explains `pane`, or every pane (tab by tab) when it is `None`;
    /// `None` when the named pane doesn't exist.
    #[must_use]
    pub fn explain(&self, pane: Option<PaneId>) -> Option<Vec<PaneExplanation>> {
        let snapshots = self.explain_snapshots(pane);
        if pane.is_some() && snapshots.is_empty() {
            return None;
        }
        let registry = ManifestRegistry::bundled();
        Some(snapshots.into_iter().map(|s| judge(s, &registry)).collect())
    }

    /// The panes to explain, with their screens, read under the lock.
    fn explain_snapshots(&self, only: Option<PaneId>) -> Vec<Snapshot> {
        let state = self.lock();
        let layout = state.layout();
        let mut snapshots = Vec::new();
        for (tab, summary) in (1..).zip(&layout.tabs) {
            for pane in summary
                .panes
                .iter()
                .filter(|p| only.is_none_or(|o| o == p.pane))
            {
                let text = state.pane(pane.pane).map(|io| io.shared().lock().text());
                snapshots.push(Snapshot {
                    explained: blank(pane, tab, state.agent_source(pane.pane)),
                    title: pane.title.clone().unwrap_or_default(),
                    text: text.unwrap_or_default(),
                });
            }
        }
        snapshots
    }
}

/// What is known without the manifest.
fn blank(pane: &PaneSummary, tab: u32, source: monarch_proto::StatusSource) -> PaneExplanation {
    PaneExplanation {
        pane: pane.pane,
        tab,
        program: pane.command.clone(),
        status: pane.agent.clone(),
        source,
        manifest: None,
        screen_state: None,
        winner: None,
        rules: Vec::new(),
    }
}

/// Adds the manifest's view of the screen, if the program has one.
fn judge(snapshot: Snapshot, registry: &ManifestRegistry) -> PaneExplanation {
    let mut explained = snapshot.explained;
    let manifest = explained
        .program
        .as_deref()
        .and_then(|p| registry.for_program(p));
    let Some(manifest) = manifest else {
        return explained;
    };
    let input = DetectionInput {
        screen: &snapshot.text,
        osc_title: &snapshot.title,
        osc_progress: "",
    };
    let (verdict, checks) = manifest.explain(input);
    explained.manifest = Some(name(manifest));
    explained.screen_state = verdict.state();
    explained.winner = verdict.rule_id().map(Into::into);
    explained.rules = checks
        .into_iter()
        .map(|c| RuleOutcome {
            id: c.rule.id.clone(),
            state: c.rule.state,
            priority: c.rule.priority,
            region: c.rule.region.to_string(),
            matched: c.matched,
            hold: c.rule.hold,
            preview: c.preview,
        })
        .collect();
    explained
}

/// `gemini 2026.10.1`, or just the id.
fn name(manifest: &Manifest) -> String {
    match &manifest.version {
        Some(version) => format!("{} {version}", manifest.id),
        None => manifest.id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentState;
    use monarch_proto::StatusSource;

    use crate::PaneSpec;
    use crate::test_support::{FakePane, context};

    #[test]
    fn panes_are_explained_with_their_manifest() {
        let ctx = context();
        let shell = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        let gemini = ctx.insert_for_test(PaneSpec::default(), FakePane::shell());
        ctx.lock().set_command(shell, Some("zsh".into()));
        ctx.lock().set_command(gemini, Some("gemini".into()));
        {
            let state = ctx.lock();
            let mut term = state.pane(gemini).expect("pane").shared().lock();
            term.feed(b"waiting (esc to cancel)", &mut Vec::new());
        }
        let all = ctx.explain(None).expect("all");
        assert_eq!(all.len(), 2);
        assert_eq!(
            (all[0].manifest.as_ref(), all[0].rules.len()),
            (None, 0),
            "a shell"
        );
        let g = &ctx.explain(Some(gemini)).expect("one")[0];
        assert!(
            g.manifest
                .as_deref()
                .is_some_and(|m| m.starts_with("gemini"))
        );
        assert_eq!(g.screen_state, Some(AgentState::Working));
        assert_eq!(g.winner.as_deref(), Some("esc_cancel_working"));
        assert!(
            g.rules
                .iter()
                .any(|r| r.matched && r.preview.contains("esc to cancel"))
        );
        assert_eq!((g.tab, g.source), (2, StatusSource::None), "its own tab");
        assert_eq!(ctx.explain(Some(monarch_core::PaneId(99))), None);
    }
}
