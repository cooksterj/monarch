//! Arguments for `monarch setup`.

use clap::Args;

/// What `monarch setup` should do to the agents' hook settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Args)]
pub struct SetupArgs {
    /// Show what would change without writing anything.
    #[arg(long)]
    pub dry_run: bool,
    /// Remove Monarch's hooks instead of installing them.
    #[arg(long)]
    pub uninstall: bool,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::{Cli, Command};

    #[test]
    fn parses_setup_flags() {
        let cli =
            Cli::try_parse_from(["monarch", "setup", "--dry-run", "--uninstall"]).expect("parses");
        let Some(Command::Setup(args)) = cli.command else {
            panic!("not setup");
        };
        assert!(args.dry_run && args.uninstall);
        assert!(Cli::try_parse_from(["monarch", "setup", "--force"]).is_err());
    }
}
