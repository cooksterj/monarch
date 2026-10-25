//! A heading in the sidebar: one git checkout, or the panes outside any.

use super::SidebarRow;

/// Panes working in one checkout (`monarch  main`), or in no repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarGroup {
    /// The checkout's top directory; `None` for panes outside a repository.
    pub worktree: Option<String>,
    /// The heading: the project's name, or `Scratch`.
    pub title: String,
    /// The branch, drawn dim beside the title.
    pub detail: Option<String>,
    /// Its panes in tab order.
    pub rows: Vec<SidebarRow>,
}

impl SidebarGroup {
    /// Heading for panes outside any repository.
    pub const SCRATCH: &'static str = "Scratch";

    /// The group for panes outside any repository.
    #[must_use]
    pub fn scratch() -> Self {
        Self {
            worktree: None,
            title: Self::SCRATCH.into(),
            detail: None,
            rows: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_has_no_checkout_and_starts_empty() {
        let group = SidebarGroup::scratch();
        assert_eq!(group.title, "Scratch");
        assert!(group.worktree.is_none() && group.rows.is_empty());
    }
}
