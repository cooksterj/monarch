//! Arguments for `monarch new`.

use clap::Args;
use monarch_core::AgentKind;

/// A new worktree with a tab working in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct NewArgs {
    /// The branch: checked out if it exists, else created. The worktree's
    /// directory is named after it, under `[projects].worktree_root`
    /// (default: `<repo>-worktrees/` beside the repository).
    pub name: String,
    /// Start this agent in the new tab instead of a shell: `claude` or
    /// `codex`.
    #[arg(long, value_name = "AGENT")]
    pub agent: Option<AgentKind>,
    /// What a new branch starts from (default: the current commit).
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    /// The session to open it in; defaults to the one you are in, else
    /// `[session].default_name`.
    #[arg(short, long)]
    pub session: Option<String>,
    /// Don't attach afterwards (from inside Monarch the new tab just
    /// appears).
    #[arg(long)]
    pub detached: bool,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrap {
        #[command(flatten)]
        args: NewArgs,
    }

    #[test]
    fn a_name_is_required() {
        let parsed = Wrap::try_parse_from(["new", "feat/x", "--agent", "claude", "--base", "main"])
            .expect("parses")
            .args;
        assert_eq!(parsed.name, "feat/x");
        assert_eq!(parsed.agent, Some(AgentKind::Claude));
        assert_eq!(parsed.base.as_deref(), Some("main"));
        assert!(Wrap::try_parse_from(["new"]).is_err());
        assert!(Wrap::try_parse_from(["new", "x", "--agent", "vim"]).is_err());
    }
}
