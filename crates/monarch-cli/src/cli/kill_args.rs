//! Arguments for `monarch kill`.

use clap::Args;

/// Which session to end.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct KillArgs {
    /// The session's name, as `monarch ls` shows it. Required, so a
    /// session is never ended by accident.
    pub name: String,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::{Cli, Command};

    #[test]
    fn the_name_is_required() {
        let cli = Cli::try_parse_from(["monarch", "kill", "work"]).expect("parse");
        assert!(matches!(cli.command, Some(Command::Kill(a)) if a.name == "work"));
        assert!(Cli::try_parse_from(["monarch", "kill"]).is_err());
    }
}
