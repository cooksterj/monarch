//! Arguments for `monarch agent read`.

use clap::Args;
use monarch_core::PaneId;

/// Which pane to read, in which session.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ReadArgs {
    /// The pane (`p3`), as `monarch agent explain` lists them.
    pub pane: PaneId,
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
        args: ReadArgs,
    }

    #[test]
    fn the_pane_is_required() {
        let one = Wrap::try_parse_from(["x", "p4", "-s", "work"])
            .expect("parse")
            .args;
        assert_eq!(
            (one.pane, one.session.as_deref()),
            (PaneId(4), Some("work"))
        );
        assert!(Wrap::try_parse_from(["x"]).is_err());
        assert!(Wrap::try_parse_from(["x", "four"]).is_err());
    }
}
