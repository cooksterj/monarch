//! What a key did in the ask composer.

use monarch_core::PaneId;

/// The result of one key in the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskStep {
    /// Still open.
    Open,
    /// Closed without sending.
    Close,
    /// Type `text` into `pane` and press Enter; the composer closes.
    Send {
        /// The agent's pane.
        pane: PaneId,
        /// The whole prompt, quote included.
        text: String,
    },
    /// Nothing sent, and why; the composer stays open.
    Refuse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_compare_by_value() {
        assert_eq!(AskStep::Refuse("a".into()), AskStep::Refuse("a".into()));
        assert_ne!(AskStep::Open, AskStep::Close);
    }
}
