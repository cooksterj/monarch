//! `session.kill`: end a session on purpose.

use serde::{Deserialize, Serialize};

/// Asks the server to hang up every pane and exit. Ending on purpose,
/// like closing the last pane, leaves nothing to restore. The answer may
/// not arrive: the server can be gone first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KillSession {}

impl KillSession {
    /// The method name.
    pub const METHOD: &'static str = "session.kill";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_no_params() {
        assert_eq!(
            serde_json::to_string(&KillSession {}).ok().as_deref(),
            Some("{}")
        );
        assert_eq!(
            serde_json::from_str::<KillSession>("{}").ok(),
            Some(KillSession {})
        );
        assert_ne!(KillSession::METHOD, crate::SessionInfo::METHOD);
    }
}
