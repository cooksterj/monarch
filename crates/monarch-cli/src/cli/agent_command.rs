//! `monarch agent …`: ask a session about its agents.

use clap::Subcommand;

use crate::{ExplainArgs, ReadArgs, WaitArgs};

/// What to ask.
#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum AgentCommand {
    /// Why Monarch shows a pane's agent in its state: where the status
    /// came from, and what the agent's detection manifest makes of the
    /// screen. Without a pane, one line per pane.
    Explain(ExplainArgs),
    /// Wait until a pane's agent reaches a state (by default, until it
    /// needs you), then print its status. Exit status 0 when reached, 2
    /// on timeout.
    Wait(WaitArgs),
    /// The last thing a pane's agent said: the final message of its last
    /// turn, from its hooks; for an agent without hooks, what its screen
    /// shows. Exit status 2 when it has said nothing yet.
    Read(ReadArgs),
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::{Cli, Command};

    use super::*;

    #[test]
    fn explain_parses_under_agent() {
        let cli = Cli::try_parse_from(["monarch", "agent", "explain"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Command::Agent(AgentCommand::Explain(_)))
        ));
        assert!(
            Cli::try_parse_from(["monarch", "agent"]).is_err(),
            "needs a verb"
        );
    }
}
