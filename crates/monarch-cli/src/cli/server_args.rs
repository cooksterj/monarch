//! Arguments for `monarch server`.

use clap::Args;

/// How to run a session server in the foreground or as a daemon.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct ServerArgs {
    /// Session name; defaults to `[session].default_name` (`main`).
    #[arg(short, long)]
    pub session: Option<String>,
    /// Shell command for an initial pane (repeatable); the shell when absent.
    #[arg(long = "pane", value_name = "CMD")]
    pub panes: Vec<String>,
    /// Detach from the terminal (used when `monarch` auto-starts a server).
    #[arg(long)]
    pub daemon: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_foreground_with_no_panes() {
        let args = ServerArgs::default();
        assert!(!args.daemon && args.panes.is_empty());
    }
}
