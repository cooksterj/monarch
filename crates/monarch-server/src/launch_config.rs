//! What the user's config says about starting panes: agent commands,
//! scrollback, extra environment and whether restore relaunches agents.

use std::collections::BTreeMap;

use monarch_core::{AgentConfig, AgentKind, Config};

use crate::PaneSpec;

/// The parts of the config a server consults when it starts a pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchConfig {
    /// How each agent is launched and resumed (`[agents.<id>]`, or the
    /// built-in when the table is missing or names no command).
    pub agents: BTreeMap<AgentKind, AgentConfig>,
    /// Scrollback lines for shell panes.
    pub scrollback: usize,
    /// Scrollback lines for panes Monarch started as an agent.
    pub agent_scrollback: usize,
    /// Extra environment for every pane (Monarch's own variables win).
    pub env: BTreeMap<String, String>,
    /// Relaunch agents when restoring a session; off, their panes come
    /// back as shells in the same directory.
    pub resume_agents: bool,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self::from(&Config::default())
    }
}

impl From<&Config> for LaunchConfig {
    fn from(config: &Config) -> Self {
        Self {
            agents: AgentKind::ALL
                .into_iter()
                .map(|kind| (kind, config.agent(kind)))
                .collect(),
            scrollback: config.terminal.scrollback,
            agent_scrollback: config.terminal.agent_scrollback,
            env: config.terminal.env.clone(),
            resume_agents: config.session.resume_agents,
        }
    }
}

impl LaunchConfig {
    /// How `kind` is launched.
    #[must_use]
    pub fn agent(&self, kind: AgentKind) -> AgentConfig {
        self.agents
            .get(&kind)
            .cloned()
            .unwrap_or_else(|| AgentConfig::for_kind(kind))
    }

    /// The scrollback `spec`'s pane gets.
    #[must_use]
    pub fn scrollback_for(&self, spec: &PaneSpec) -> usize {
        if spec.agent.is_some() {
            self.agent_scrollback
        } else {
            self.scrollback
        }
    }

    /// Which agent a foreground program is, by its name: the configured
    /// commands (a wrapper named in `command`) and the built-in names.
    #[must_use]
    pub fn programs(&self) -> BTreeMap<String, AgentKind> {
        let mut out: BTreeMap<String, AgentKind> = AgentKind::ALL
            .into_iter()
            .map(|kind| (kind.program().to_owned(), kind))
            .collect();
        for (kind, agent) in &self.agents {
            out.entry(agent.program_name().to_owned()).or_insert(*kind);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_the_default_config() {
        let launch = LaunchConfig::default();
        assert_eq!(launch.scrollback, 10_000);
        assert_eq!(launch.agent_scrollback, 5_000);
        assert!(launch.env.is_empty() && launch.resume_agents);
        assert_eq!(launch.agent(AgentKind::Claude).command, "claude");
    }

    #[test]
    fn agents_get_the_smaller_scrollback() {
        let launch = LaunchConfig {
            scrollback: 9,
            agent_scrollback: 4,
            ..LaunchConfig::default()
        };
        let agent = PaneSpec::agent(AgentKind::Codex, &launch.agent(AgentKind::Codex));
        assert_eq!(launch.scrollback_for(&agent), 4);
        assert_eq!(launch.scrollback_for(&PaneSpec::default()), 9);
    }

    #[test]
    fn a_configured_wrapper_is_known_by_name_beside_the_builtins() {
        let mut config = Config::default();
        config.agents.get_mut("claude").expect("claude").command = "/opt/my-claude".into();
        let programs = LaunchConfig::from(&config).programs();
        assert_eq!(programs.get("my-claude"), Some(&AgentKind::Claude));
        assert_eq!(programs.get("claude"), Some(&AgentKind::Claude));
        assert_eq!(programs.get("codex"), Some(&AgentKind::Codex));
        assert_eq!(programs.get("zsh"), None);
    }

    #[test]
    fn a_missing_kind_falls_back_to_the_builtin() {
        let launch = LaunchConfig {
            agents: BTreeMap::new(),
            ..LaunchConfig::default()
        };
        assert_eq!(
            launch.agent(AgentKind::Codex),
            AgentConfig::for_kind(AgentKind::Codex)
        );
    }
}
