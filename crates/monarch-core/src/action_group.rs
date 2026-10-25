//! The areas the help page sorts actions into.

/// What an action works on, for grouping in the help page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionGroup {
    /// Splitting, focusing, moving and sizing panes.
    Panes,
    /// Opening and switching tabs.
    Tabs,
    /// Agents that need you.
    Agents,
    /// Git checkouts and worktrees.
    Projects,
    /// The session and the client: detach, switch, pickers, settings.
    Session,
}

impl ActionGroup {
    /// Every group, in the order the help page shows them.
    pub const ALL: [Self; 5] = [
        Self::Panes,
        Self::Tabs,
        Self::Agents,
        Self::Projects,
        Self::Session,
    ];

    /// The heading.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Panes => "Panes",
            Self::Tabs => "Tabs",
            Self::Agents => "Agents",
            Self::Projects => "Projects",
            Self::Session => "Session",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_are_listed_once_in_order() {
        let mut sorted = ActionGroup::ALL;
        sorted.sort();
        assert_eq!(sorted, ActionGroup::ALL);
        assert_eq!(ActionGroup::Agents.title(), "Agents");
    }
}
