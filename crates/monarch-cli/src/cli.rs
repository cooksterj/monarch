//! The argument grammar for the `monarch` binary.

mod attach_args;
mod server_args;

use clap::{Parser, Subcommand};

pub use attach_args::AttachArgs;
pub use server_args::ServerArgs;

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
    /// Attach to a session, starting its server if needed.
    Attach(AttachArgs),
    /// Run a session server (normally started for you by `monarch`).
    Server(ServerArgs),
    /// List what each key after the prefix does.
    Keys,
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
    fn parses_attach_and_server() {
        let cli = Cli::try_parse_from(["monarch", "attach", "-s", "work"]).expect("parses");
        assert!(
            matches!(cli.command, Some(Command::Attach(a)) if a.session.as_deref() == Some("work"))
        );
        let cli = Cli::try_parse_from([
            "monarch", "server", "--pane", "htop", "--pane", "top", "--daemon",
        ])
        .expect("parses");
        let Some(Command::Server(args)) = cli.command else {
            panic!("not server");
        };
        assert_eq!(args.panes, ["htop", "top"]);
        assert!(args.daemon);
    }

    #[test]
    fn no_args_means_no_command() {
        let cli = Cli::try_parse_from(["monarch"]).expect("parses");
        assert!(cli.command.is_none());
    }
}
