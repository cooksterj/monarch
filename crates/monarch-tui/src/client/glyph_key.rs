//! Which sidebar glyph an animation belongs to.

use monarch_core::PaneId;

/// A pane's agent, or one sub-agent inside it (by the agent's id for
/// it): each glyph keeps its own one-off animation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GlyphKey {
    /// The agent running in a pane.
    Pane(PaneId),
    /// A sub-agent of that pane's agent.
    Subagent(PaneId, String),
}

impl From<PaneId> for GlyphKey {
    fn from(pane: PaneId) -> Self {
        Self::Pane(pane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_and_its_subagents_are_different_glyphs() {
        let pane = GlyphKey::from(PaneId(2));
        assert_eq!(pane, GlyphKey::Pane(PaneId(2)));
        let child = GlyphKey::Subagent(PaneId(2), "a1".into());
        assert_ne!(pane, child);
        assert_ne!(child, GlyphKey::Subagent(PaneId(3), "a1".into()));
        assert!(pane < child, "a pane sorts before its sub-agents");
    }
}
