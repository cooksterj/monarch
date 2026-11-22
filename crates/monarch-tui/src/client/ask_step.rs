//! What a key did in the ask composer.

use monarch_core::{AgentKind, PaneId};

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
    /// Queue `text` for `pane`'s agent, which isn't at its prompt: the
    /// server types it in once it is. The composer closes.
    Queue {
        /// The agent's pane.
        pane: PaneId,
        /// The whole prompt, quote included.
        text: String,
    },
    /// Start each of `kinds` beside the pane, each asked `text` first;
    /// the composer closes.
    Start {
        /// The agents to start, one pane each.
        kinds: Vec<AgentKind>,
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
