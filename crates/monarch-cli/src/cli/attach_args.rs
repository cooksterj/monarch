//! Arguments for `monarch attach` (and a bare `monarch`).

use clap::Args;

/// Which session to attach to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
pub struct AttachArgs {
    /// Session name; defaults to `[session].default_name` (`main`).
    #[arg(short, long)]
    pub session: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_session() {
        assert_eq!(AttachArgs::default().session, None);
    }
}
