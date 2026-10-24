//! `[notifications]`: when and how agents get your attention.

use serde::{Deserialize, Serialize};

/// Notification rules. Quiet by default: nothing fires for the pane you are
/// already looking at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationsConfig {
    /// Master switch.
    pub enabled: bool,
    /// Also notify for the focused pane.
    pub focused_pane: bool,
    /// Notify when an agent becomes blocked.
    pub on_blocked: bool,
    /// Notify when an agent finishes a turn.
    pub on_done: bool,
    /// Notify when an agent errors or exits.
    pub on_error: bool,
    /// Forward to the host terminal as OSC 9/99/777 (desktop notifications,
    /// works over SSH).
    pub host_osc: bool,
    /// Ring the host terminal bell.
    pub bell: bool,
    /// Optional command run with the notification as JSON on stdin.
    pub command: Option<Vec<String>>,
    /// Bursts within this window are batched into one notification.
    pub batch_ms: u64,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            focused_pane: false,
            on_blocked: true,
            on_done: true,
            on_error: true,
            host_osc: true,
            bell: false,
            command: None,
            batch_ms: 1500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_pane_is_quiet_by_default() {
        assert!(!NotificationsConfig::default().focused_pane);
    }
}
