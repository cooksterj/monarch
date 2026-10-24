//! What a key did to an open picker.

use monarch_core::Action;

/// The result of one key in a picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerStep {
    /// Still open (the query or highlight may have changed).
    Open,
    /// Closed without choosing.
    Close,
    /// Closed by choosing a row: run this.
    Run(Action),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_eq!(
            PickerStep::Run(Action::NewTab),
            PickerStep::Run(Action::NewTab)
        );
        assert_ne!(PickerStep::Open, PickerStep::Close);
    }
}
