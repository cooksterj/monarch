//! `monarch pane …`: act on a session's panes.

use clap::Subcommand;

use crate::SendArgs;

/// What to do.
#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum PaneCommand {
    /// Type text into a pane as if you had, optionally pressing Enter:
    /// hand a prompt to an agent you started, or run a command in a
    /// shell. Multi-line text arrives as one paste when the program
    /// accepts pastes.
    Send(SendArgs),
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::{Cli, Command};

    use super::*;

    #[test]
    fn send_parses_under_pane() {
        let cli =
            Cli::try_parse_from(["monarch", "pane", "send", "p3", "hi", "--enter"]).expect("parse");
        let Some(Command::Pane(PaneCommand::Send(args))) = cli.command else {
            panic!("not pane send");
        };
        assert!(args.enter && args.text.as_deref() == Some("hi"));
        assert!(
            Cli::try_parse_from(["monarch", "pane"]).is_err(),
            "needs a verb"
        );
    }
}
