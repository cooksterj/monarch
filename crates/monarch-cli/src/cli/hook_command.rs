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

/// The Claude Code events whose hooks report state.
const CLAUDE_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// The Codex events whose hooks report state.
const CODEX_EVENTS: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "Stop",
    "Interrupt",
];

/// Seconds an agent waits for a hook; the hook finishes in milliseconds.
const TIMEOUT: u32 = 5;
/// Codex caps these events' hooks at 3 s and warns about longer ones.
const CODEX_SHORT: (&[&str], u32) = (&["SessionEnd", "Interrupt"], 3);

impl HookCommand {
    /// Every agent with hooks.
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    /// The subcommand's name: `monarch hook <name>`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// The agent's name for people.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }

    /// The hook events `monarch setup` installs for this agent.
    #[must_use]
    pub fn events(self) -> &'static [&'static str] {
        match self {
            Self::Claude => CLAUDE_EVENTS,
            Self::Codex => CODEX_EVENTS,
        }
    }

    /// The timeout, in seconds, to give `event`'s hook.
    #[must_use]
    pub fn timeout(self, event: &str) -> u32 {
        match self {
            Self::Codex if CODEX_SHORT.0.contains(&event) => CODEX_SHORT.1,
            _ => TIMEOUT,
        }
    }

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

    #[test]
    fn names_match_the_subcommands() {
        for agent in HookCommand::ALL {
            let cli = Cli::try_parse_from(["monarch", "hook", agent.name()]).expect("parses");
            assert!(matches!(cli.command, Some(Command::Hook(c)) if c == agent));
        }
    }

    #[test]
    fn codex_caps_some_timeouts() {
        assert_eq!(HookCommand::Codex.timeout("Stop"), 5);
        assert_eq!(HookCommand::Codex.timeout("SessionEnd"), 3);
        assert_eq!(HookCommand::Claude.timeout("SessionEnd"), 5);
        assert!(HookCommand::Claude.events().contains(&"StopFailure"));
        assert!(!HookCommand::Codex.events().contains(&"StopFailure"));
    }
}
