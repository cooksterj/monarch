//! The root of Monarch's user configuration (`~/.config/monarch/config.toml`).
//!
//! Each `[section]` is its own type in `config/`. Every field has a default,
//! so an empty or missing file is a valid configuration.

mod agents;
mod keys;
mod loader;
mod mouse_mode;
mod notifications;
mod projects;
mod session;
mod terminal;
mod theme;
mod ui;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use agents::AgentConfig;
pub use keys::KeysConfig;
pub use loader::{ConfigLoader, LoadedConfig};
pub use mouse_mode::MouseMode;
pub use notifications::NotificationsConfig;
pub use projects::ProjectsConfig;
pub use session::SessionConfig;
pub use terminal::TerminalConfig;
pub use theme::ThemeConfig;
pub use ui::UiConfig;

/// The complete user configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// `[keys]`: prefix chord and binding preset.
    pub keys: KeysConfig,
    /// `[ui]`: sidebar, mouse and layout chrome.
    pub ui: UiConfig,
    /// `[theme]`: colour theme selection.
    pub theme: ThemeConfig,
    /// `[notifications]`: when and how agents get your attention.
    pub notifications: NotificationsConfig,
    /// `[agents.<id>]`: how to launch and resume each agent.
    pub agents: BTreeMap<String, AgentConfig>,
    /// `[terminal]`: shell and scrollback for panes.
    pub terminal: TerminalConfig,
    /// `[projects]`: git worktree placement.
    pub projects: ProjectsConfig,
    /// `[session]`: session naming and restore behaviour.
    pub session: SessionConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            keys: KeysConfig::default(),
            ui: UiConfig::default(),
            theme: ThemeConfig::default(),
            notifications: NotificationsConfig::default(),
            agents: AgentConfig::builtin(),
            terminal: TerminalConfig::default(),
            projects: ProjectsConfig::default(),
            session: SessionConfig::default(),
        }
    }
}

impl Config {
    /// The default configuration rendered as commented-free TOML, used by
    /// `monarch config default` so docs and defaults never diverge.
    #[must_use]
    pub fn default_toml() -> String {
        toml::to_string_pretty(&Self::default()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_toml_round_trips() {
        let text = Config::default_toml();
        let parsed: Config = toml::from_str(&text).expect("default config parses");
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn empty_file_is_the_default() {
        let parsed: Config = toml::from_str("").expect("empty config parses");
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn defaults_include_claude_and_codex() {
        let config = Config::default();
        assert!(config.agents.contains_key("claude"));
        assert!(config.agents.contains_key("codex"));
    }
}
