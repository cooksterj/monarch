//! Arguments for `monarch pane send`.

use clap::Args;
use monarch_core::PaneId;

/// Which pane, what to type, and whether to press Enter.
#[derive(Debug, Clone, PartialEq, Args)]
pub struct SendArgs {
    /// The pane (`p3`), as `monarch agent explain` lists them.
    pub pane: PaneId,
    /// What to type; read from standard input when left out (for long or
    /// multi-line prompts).
    pub text: Option<String>,
    /// Press Enter afterwards: submits a prompt or runs a command.
    #[arg(long)]
    pub enter: bool,
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
        args: SendArgs,
    }

    #[test]
    fn text_is_optional_and_the_pane_must_be_one() {
        let w = Wrap::try_parse_from(["x", "4"]).expect("parse");
        assert_eq!(
            (w.args.pane, w.args.text, w.args.enter),
            (PaneId(4), None, false)
        );
        assert!(Wrap::try_parse_from(["x", "pane-four"]).is_err());
        let w = Wrap::try_parse_from(["x", "p2", "-s", "work", "two words"]).expect("parse");
        assert_eq!(w.args.session.as_deref(), Some("work"));
        assert_eq!(w.args.text.as_deref(), Some("two words"));
    }
}
