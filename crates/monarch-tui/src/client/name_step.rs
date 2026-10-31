//! What a key did in a rename prompt.

/// The result of one key in a rename prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameStep {
    /// Still open.
    Open,
    /// Closed without renaming.
    Close,
    /// Close and give this name (blank: the automatic one).
    Rename(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_carry_their_name() {
        assert_eq!(NameStep::Rename("a".into()), NameStep::Rename("a".into()));
        assert_ne!(NameStep::Rename(String::new()), NameStep::Close);
    }
}
