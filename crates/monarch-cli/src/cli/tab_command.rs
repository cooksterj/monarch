//! `monarch tab …`: act on a session's tabs.

use clap::Subcommand;

use crate::RenameArgs;

/// What to do.
#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum TabCommand {
    /// Name a tab, as `prefix ,` does: the tab you run it in, else the one
    /// shown, or `--tab N`. A blank name (`""`) gives the automatic one
    /// back.
    Rename(RenameArgs),
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::{Cli, Command};

    #[test]
    fn rename_parses_under_tab() {
        let cli =
            Cli::try_parse_from(["monarch", "tab", "rename", "api", "--tab", "2"]).expect("parse");
        let Some(Command::Tab(TabCommand::Rename(args))) = cli.command else {
            panic!("not tab rename");
        };
        assert_eq!((args.name.as_str(), args.tab), ("api", Some(2)));
        assert!(
            Cli::try_parse_from(["monarch", "tab"]).is_err(),
            "needs a verb"
        );
    }
}
