//! One row of a picker: what it says and what choosing it does.

use monarch_core::Action;

/// A row: the text searched and shown, a hint drawn dim on the right
/// (a key, `here`), and the action it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerItem {
    /// Shown and searched.
    pub label: String,
    /// Shown dim at the right; not searched.
    pub detail: String,
    /// Run when chosen.
    pub action: Action,
}

impl PickerItem {
    /// A row for `action`, labelled by its name (`split right`).
    #[must_use]
    pub fn for_action(action: Action, detail: impl Into<String>) -> Self {
        Self {
            label: action.to_string().replace('-', " "),
            detail: detail.into(),
            action,
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::SplitAxis;

    use super::*;

    #[test]
    fn actions_are_labelled_by_name() {
        let item = PickerItem::for_action(Action::Split(SplitAxis::Columns), "%");
        assert_eq!(item.label, "split right");
        assert_eq!(item.detail, "%");
        assert_eq!(
            PickerItem::for_action(Action::SelectTab(3), "").label,
            "tab 3"
        );
    }
}
