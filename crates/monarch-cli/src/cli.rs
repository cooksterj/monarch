//! The argument grammar for the `monarch` binary.

use clap::{Parser, Subcommand};

/// An attention-first terminal multiplexer for AI coding agents.
#[derive(Debug, Parser)]
#[command(name = "monarch", version, about, propagate_version = true)]
pub struct Cli {
    /// The command to run; with none, Monarch attaches to the default session.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level commands. New commands are added milestone by milestone.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect or validate the configuration file.
    #[command(subcommand)]
    Config(ConfigCommand),
}

/// `monarch config …`
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print the path Monarch reads its config from.
    Path,
    /// Print the default configuration as TOML.
    Default,
    /// Validate the config file and report unknown keys.
    Check,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn grammar_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_config_check() {
        let cli = Cli::try_parse_from(["monarch", "config", "check"]).expect("parses");
        assert!(matches!(
            cli.command,
            Some(Command::Config(ConfigCommand::Check))
        ));
    }

    #[test]
    fn no_args_means_no_command() {
        let cli = Cli::try_parse_from(["monarch"]).expect("parses");
        assert!(cli.command.is_none());
    }
}
