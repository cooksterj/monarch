//! Arguments for `monarch agent wait`.

use clap::Args;
use monarch_core::{AgentState, PaneId};

/// Which pane to wait on, for what, and for how long.
#[derive(Debug, Clone, PartialEq, Args)]
pub struct WaitArgs {
    /// The pane (`p3`), as `monarch agent explain` lists them.
    pub pane: PaneId,
    /// The states to wait for, comma-separated: working, blocked, idle,
    /// done, error. Default: blocked,done,error (it needs you).
    #[arg(long, value_delimiter = ',', value_name = "STATES")]
    pub until: Vec<AgentState>,
    /// Give up after this many seconds (exit status 2); default: wait
    /// as long as it takes.
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<f64>,
    /// The session; defaults to the one you are in, else
    /// `[session].default_name`.
    #[arg(short, long)]
    pub session: Option<String>,
}

impl WaitArgs {
    /// The states waited for, with the default filled in.
    #[must_use]
    pub fn states(&self) -> Vec<AgentState> {
        if self.until.is_empty() {
            AgentState::ALL
                .into_iter()
                .filter(|s| s.needs_attention())
                .collect()
        } else {
            self.until.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrap {
        #[command(flatten)]
        args: WaitArgs,
    }

    fn parse(args: &[&str]) -> Result<WaitArgs, clap::Error> {
        Wrap::try_parse_from(std::iter::once("x").chain(args.iter().copied())).map(|w| w.args)
    }

    #[test]
    fn states_default_to_needing_attention() {
        let plain = parse(&["p3"]).expect("parse");
        assert_eq!(plain.pane, PaneId(3));
        assert_eq!(
            plain.states(),
            [AgentState::Blocked, AgentState::Error, AgentState::Done]
        );
        let some = parse(&["3", "--until", "idle,working", "--timeout", "1.5"]).expect("parse");
        assert_eq!(some.states(), [AgentState::Idle, AgentState::Working]);
        assert_eq!(some.timeout, Some(1.5));
        assert!(parse(&["p3", "--until", "busy"]).is_err());
        assert!(parse(&[]).is_err(), "the pane is required");
    }
}
