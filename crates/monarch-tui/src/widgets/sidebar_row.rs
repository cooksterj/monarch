//! One pane as the sidebar lists it.

use monarch_core::PaneId;

/// A pane under its project heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarRow {
    /// The pane a click focuses.
    pub pane: PaneId,
    /// Its tab number and name (`2 vim`).
    pub label: String,
    /// Where in the checkout it works (`src`), or its directory outside
    /// one; drawn dim after the label.
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
