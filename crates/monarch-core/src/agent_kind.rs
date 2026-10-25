//! Which coding agent a pane runs, when Monarch launched it as one.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// An agent Monarch knows how to launch and resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// Claude Code (`claude`).
    Claude,
    /// OpenAI Codex CLI (`codex`).
    Codex,
}

impl AgentKind {
    /// Every kind, in the order help text lists them.
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    /// The name used on the command line and in files.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// Every name, comma-separated, for help and errors.
    #[must_use]
    pub fn choices() -> String {
        Self::ALL.map(Self::name).join(", ")
    }

    /// The program to run.
    #[must_use]
    pub fn program(self) -> &'static str {
        self.name()
    }

    /// The kind whose program is `command`, if any.
    #[must_use]
    pub fn for_program(command: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.program() == command)
    }

    /// How the agent calls itself, for notices.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for AgentKind {
    type Err = CoreError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name().eq_ignore_ascii_case(text))
            .ok_or_else(|| CoreError::UnknownAgent(text.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_name_their_kind() {
        assert_eq!(AgentKind::for_program("codex"), Some(AgentKind::Codex));
        assert_eq!(AgentKind::for_program("zsh"), None);
    }

    #[test]
    fn names_round_trip() {
        for kind in AgentKind::ALL {
            assert_eq!(kind.name().parse::<AgentKind>().expect("parse"), kind);
            assert_eq!(kind.to_string(), kind.name());
        }
        assert_eq!("Claude".parse::<AgentKind>().ok(), Some(AgentKind::Claude));
    }

    #[test]
    fn unknown_agents_are_rejected_with_the_choices() {
        let err = "gemini".parse::<AgentKind>().expect_err("unknown");
        assert!(err.to_string().contains("claude, codex"), "{err}");
    }

    #[test]
    fn serialises_as_its_name() {
        assert_eq!(
            serde_json::to_string(&AgentKind::Codex).expect("json"),
            "\"codex\""
        );
    }
}
