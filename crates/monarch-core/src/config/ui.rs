//! `[ui]`: sidebar, mouse and chrome settings.

use serde::{Deserialize, Serialize};

use super::MouseMode;

/// User-interface chrome configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Show the project/agent sidebar.
    pub sidebar: bool,
    /// Sidebar width in columns.
    pub sidebar_width: u16,
    /// Hide the sidebar automatically below this terminal width.
    pub sidebar_min_columns: u16,
    /// Mouse capture policy.
    pub mouse: MouseMode,
    /// Host window title format; `{session}`, `{pane}` and `{title}` expand.
    pub window_title: String,
    /// Animate agent-state glyphs (pulsing, fading); off draws them still.
    pub animations: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            sidebar: true,
            sidebar_width: 32,
            sidebar_min_columns: 100,
            mouse: MouseMode::Auto,
            window_title: "{title} — monarch:{session}".into(),
            animations: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_defaults_to_auto() {
        assert_eq!(UiConfig::default().mouse, MouseMode::Auto);
        assert!(UiConfig::default().animations);
    }
}
