//! What a key did in the rename-tab prompt.

/// The result of one key in the rename-tab prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabNameStep {
    /// Still open.
    Open,
    /// Closed without renaming.
    Close,
    /// Close and give the tab this name (blank: its automatic name).
    Rename(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_carry_their_name() {
        assert_eq!(
            TabNameStep::Rename("a".into()),
            TabNameStep::Rename("a".into())
        );
        assert_ne!(TabNameStep::Rename(String::new()), TabNameStep::Close);
    }
}
