//! The root of Monarch's user configuration (`~/.config/monarch/config.toml`).
//!
//! Each `[section]` is its own type in `config/`. Every field has a default,
//! so an empty or missing file is a valid configuration.

mod agents;
mod keys;
mod loader;
mod mouse_mode;
mod projects;
mod session;
mod terminal;
mod theme;
mod ui;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::AgentKind;

pub use agents::{AgentConfig, SESSION_ID};
pub use keys::KeysConfig;
pub use loader::{ConfigLoader, LoadedConfig};
pub use mouse_mode::MouseMode;
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

    /// How to launch `kind`: its `[agents.<id>]` table, or the built-in
    /// one when the table is missing or names no command.
    #[must_use]
    pub fn agent(&self, kind: AgentKind) -> AgentConfig {
        self.agents
            .get(kind.name())
            .filter(|agent| !agent.command.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| AgentConfig::for_kind(kind))
    }

    /// Problems in the `[agents]` tables (each falls back to a built-in).
    #[must_use]
    pub fn agent_warnings(&self) -> Vec<String> {
        self.agents
            .iter()
            .flat_map(|(id, agent)| agent.warnings(id))
            .collect()
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

    #[test]
    fn agent_lookup_honours_the_table_and_falls_back() {
        let mut config = Config::default();
        config.agents.get_mut("claude").expect("claude").command = "my-claude".into();
        assert_eq!(config.agent(AgentKind::Claude).command, "my-claude");
        config.agents.get_mut("codex").expect("codex").command = String::new();
        assert_eq!(
            config.agent(AgentKind::Codex),
            AgentConfig::for_kind(AgentKind::Codex)
        );
        config.agents.remove("claude");
        assert_eq!(config.agent(AgentKind::Claude).command, "claude");
        assert_eq!(config.agent_warnings().len(), 1, "the empty command");
    }
}
