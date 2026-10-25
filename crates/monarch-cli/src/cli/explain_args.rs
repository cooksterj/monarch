//! Arguments for `monarch agent explain`.

use clap::Args;
use monarch_core::PaneId;

/// Which pane, in which session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct ExplainArgs {
    /// The pane (`p3`, or `3`), as the list without one shows; omit it to
    /// list every pane.
    pub pane: Option<PaneId>,
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
        args: ExplainArgs,
    }

    #[test]
    fn the_pane_is_optional_and_checked() {
        let one = Wrap::try_parse_from(["x", "p3", "-s", "work"])
            .expect("parse")
            .args;
        assert_eq!(
            (one.pane, one.session.as_deref()),
            (Some(PaneId(3)), Some("work"))
        );
        assert_eq!(
            Wrap::try_parse_from(["x"]).expect("parse").args,
            ExplainArgs::default()
        );
        assert!(Wrap::try_parse_from(["x", "pane-three"]).is_err());
    }
}
