//! The lifecycle state of an agent running in a pane.

use serde::{Deserialize, Serialize};

/// What an agent in a pane is doing, as fused from hooks, OSC
/// notifications, screen manifests and quiescence timers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentState {
    /// Actively processing a turn.
    Working,
    /// Waiting on the user (permission prompt, question, dialog).
    Blocked,
    /// Prompt visible, nothing happening.
    Idle,
    /// Finished a turn the user has not looked at yet.
    Done,
    /// The last turn failed.
    Error,
    /// The agent process exited.
    Exited,
    /// A plain shell or unrecognised program.
    #[default]
    Unknown,
}

impl AgentState {
    /// Every state, in sidebar sort order (most urgent first).
    pub const ALL: [Self; 7] = [
        Self::Blocked,
        Self::Error,
        Self::Done,
        Self::Working,
        Self::Idle,
        Self::Exited,
        Self::Unknown,
    ];

    /// Whether this state belongs in the Attention Inbox.
    #[must_use]
    pub fn needs_attention(self) -> bool {
        matches!(self, Self::Blocked | Self::Done | Self::Error)
    }

    /// Single-cell glyph shown in the sidebar and status bar.
    #[must_use]
    pub fn glyph(self) -> char {
        match self {
            Self::Working => '●',
            Self::Blocked => '◆',
            Self::Idle => '○',
            Self::Done => '✓',
            Self::Error => '✗',
            Self::Exited => '■',
            Self::Unknown => '$',
        }
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Working => "Working",
            Self::Blocked => "Blocked",
            Self::Idle => "Idle",
            Self::Done => "Done",
            Self::Error => "Error",
            Self::Exited => "Exited",
            Self::Unknown => "Shell",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_blocked_done_and_error_need_attention() {
        let needy: Vec<_> = AgentState::ALL
            .into_iter()
            .filter(|s| s.needs_attention())
            .collect();
        assert_eq!(
            needy,
            [AgentState::Blocked, AgentState::Error, AgentState::Done]
        );
    }

    #[test]
    fn glyphs_are_unique() {
        let mut glyphs: Vec<char> = AgentState::ALL.iter().map(|s| s.glyph()).collect();
        glyphs.sort_unstable();
        glyphs.dedup();
        assert_eq!(glyphs.len(), AgentState::ALL.len());
    }

    #[test]
    fn serde_uses_snake_case() {
        let json = serde_json::to_string(&AgentState::Blocked).ok();
        assert_eq!(json.as_deref(), Some("\"blocked\""));
    }
}
