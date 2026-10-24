//! `[terminal]`: shell, scrollback and environment for panes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Pane terminal configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalConfig {
    /// Shell for new panes; `None` uses `$SHELL`, then `/bin/sh`.
    pub shell: Option<String>,
    /// Scrollback lines for shell panes.
    pub scrollback: usize,
    /// Scrollback lines for agent panes (agents redraw a lot; keep memory down).
    pub agent_scrollback: usize,
    /// Extra environment variables for every pane.
    pub env: BTreeMap<String, String>,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: None,
            scrollback: 10_000,
            agent_scrollback: 5_000,
            env: BTreeMap::new(),
        }
    }
}

impl TerminalConfig {
    /// The shell to launch, resolving `None` through `$SHELL`.
    #[must_use]
    pub fn resolved_shell(&self, env_shell: Option<&str>) -> String {
        self.shell
            .clone()
            .or_else(|| env_shell.filter(|s| !s.is_empty()).map(str::to_owned))
            .unwrap_or_else(|| "/bin/sh".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_resolution_order() {
        let mut t = TerminalConfig::default();
        assert_eq!(t.resolved_shell(None), "/bin/sh");
        assert_eq!(t.resolved_shell(Some("")), "/bin/sh");
        assert_eq!(t.resolved_shell(Some("/bin/zsh")), "/bin/zsh");
        t.shell = Some("fish".into());
        assert_eq!(t.resolved_shell(Some("/bin/zsh")), "fish");
    }
}
