//! The argument grammar for the `monarch` binary.

mod agent_command;
mod attach_args;
mod explain_args;
mod hook_command;
mod kill_args;
mod new_args;
mod server_args;
mod setup_args;

use clap::{Parser, Subcommand};

pub use agent_command::AgentCommand;
pub use attach_args::AttachArgs;
pub use explain_args::ExplainArgs;
pub use hook_command::HookCommand;
pub use kill_args::KillArgs;
pub use new_args::NewArgs;
pub use server_args::ServerArgs;
pub use setup_args::SetupArgs;

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
    /// Make a git worktree for a branch and open a tab in it, optionally
    /// running an agent.
    New(NewArgs),
    /// List running sessions: panes, tabs and whether one is attached.
    #[command(visible_alias = "list")]
    Ls,
    /// End a session: hang up its panes and stop its server. It is not
    /// restored afterwards.
    Kill(KillArgs),
    /// Ask a session about its agents.
    #[command(subcommand)]
    Agent(AgentCommand),
    /// Run a session server (normally started for you by `monarch`).
    Server(ServerArgs),
    /// List what each key after the prefix does.
    Keys,
    /// Install Monarch's hooks into Claude Code and Codex (safe to re-run).
    Setup(SetupArgs),
    /// Report an agent's hook event to its pane (called by agents' hooks).
    #[command(subcommand)]
    Hook(HookCommand),
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
