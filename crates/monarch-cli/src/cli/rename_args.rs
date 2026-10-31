//! Arguments for `monarch tab rename`.

use clap::Args;

/// The new name, which tab, and in which session.
#[derive(Debug, Clone, PartialEq, Args)]
pub struct RenameArgs {
    /// The new name; `""` gives the tab its automatic name back.
    pub name: String,
    /// The tab's number, as the tab bar shows it (from 1). Defaults to
    /// the tab you run this in, else the one shown.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    pub tab: Option<u16>,
    /// The session; defaults to the one you are in, else
    /// `[session].default_name`.
    #[arg(short, long)]
    pub session: Option<String>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrap {
        #[command(flatten)]
        args: RenameArgs,
    }

    #[test]
    fn a_name_is_needed_and_may_be_blank() {
        let w = Wrap::try_parse_from(["x", ""]).expect("parse");
        assert_eq!((w.args.name.as_str(), w.args.tab), ("", None));
        assert!(Wrap::try_parse_from(["x"]).is_err());
        let w = Wrap::try_parse_from(["x", "two words", "-s", "work"]).expect("parse");
        assert_eq!(w.args.session.as_deref(), Some("work"));
    }

    #[test]
    fn tabs_count_from_one() {
        assert!(Wrap::try_parse_from(["x", "a", "--tab", "0"]).is_err());
        assert!(Wrap::try_parse_from(["x", "a", "--tab", "two"]).is_err());
    }
}
