//! `monarch hook …`: entry points agents' hook systems call.

use clap::Subcommand;

/// Which agent's hook is calling. Each reads that agent's hook JSON on
/// stdin and reports to the pane's session; outside Monarch (no
/// `MONARCH_PANE`) it does nothing, so a global hook install is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum HookCommand {
    /// A Claude Code hook (any event).
    Claude,
    /// A Codex hook (any event).
    Codex,
}

impl HookCommand {
    /// The agent whose hook this is.
    #[must_use]
    pub fn agent(self) -> monarch_core::AgentKind {
        match self {
            Self::Claude => monarch_core::AgentKind::Claude,
            Self::Codex => monarch_core::AgentKind::Codex,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::{Cli, Command};

    use super::*;

    #[test]
    fn parses_hook_claude() {
        let cli = Cli::try_parse_from(["monarch", "hook", "claude"]).expect("parses");
        assert!(matches!(
            cli.command,
            Some(Command::Hook(HookCommand::Claude))
        ));
        let codex = Cli::try_parse_from(["monarch", "hook", "codex"]).expect("parses");
        assert!(
            matches!(codex.command, Some(Command::Hook(c)) if c.agent() == monarch_core::AgentKind::Codex)
        );
        assert!(Cli::try_parse_from(["monarch", "hook", "gemini"]).is_err());
    }
}
