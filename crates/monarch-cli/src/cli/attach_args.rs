//! Arguments for `monarch attach` (and a bare `monarch`).

use clap::Args;
use monarch_core::AgentKind;

/// Which session to attach to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct AttachArgs {
    /// Session name; defaults to `[session].default_name` (`main`).
    #[arg(short, long)]
    pub session: Option<String>,
    /// Start the new session with an agent pane (repeatable): `claude` or
    /// `codex`. Only for a session that isn't running yet.
    #[arg(long = "agent", value_name = "AGENT")]
    pub agents: Vec<AgentKind>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_session() {
        assert_eq!(AttachArgs::default().session, None);
    }
}
