//! One pane as the sidebar lists it.

use monarch_core::{AgentState, PaneId};

use super::GlyphLook;

/// A pane under its project heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarRow {
    /// The pane a click focuses.
    pub pane: PaneId,
    /// Its tab number and name (`2 vim`).
    pub label: String,
    /// Its agent's state, drawn as a glyph before the label (`$` for a
    /// plain shell).
    pub state: AgentState,
    /// How its glyph looks this frame (animated, coloured); `None` draws
    /// the state's plain glyph.
    pub look: Option<GlyphLook>,
    /// Its agent's activity, or where in the checkout it works (`src`),
    /// or its directory outside one; drawn dim after the label.
    pub detail: Option<String>,
    /// It is the focused pane.
    pub focused: bool,
    /// It is in the shown tab; others are drawn dim.
    pub shown: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_compare_by_content() {
        let row = SidebarRow {
            pane: PaneId(1),
            label: "1 zsh".into(),
            state: AgentState::Unknown,
            look: None,
            detail: None,
            focused: true,
            shown: true,
        };
        let mut other = row.clone();
        assert_eq!(row, other);
        other.focused = false;
        assert_ne!(row, other);
    }
}
