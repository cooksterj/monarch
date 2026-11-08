//! What a key did in the launch picker.

use monarch_core::AgentKind;

/// The result of one key in the launch picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStep {
    /// Still open.
    Open,
    /// Closed without choosing.
    Close,
    /// Close and start this agent in a new tab.
    Launch(AgentKind),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launches_carry_their_agent() {
        assert_eq!(
            LaunchStep::Launch(AgentKind::Codex),
            LaunchStep::Launch(AgentKind::Codex)
        );
        assert_ne!(LaunchStep::Launch(AgentKind::Claude), LaunchStep::Close);
    }
}
