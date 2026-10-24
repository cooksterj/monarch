//! `monarch attach` (and a bare `monarch`): connect to a session.

use std::io::Write;
use std::process::ExitCode;

use anyhow::Context;
use monarch_core::Config;
use monarch_server::SocketPath;
use monarch_tui::{AttachApp, AttachOptions, AttachOutcome};

use super::auto_start::ensure_server;
use crate::AttachArgs;

/// Attaches, starting the server first if it is not running.
pub(crate) fn run(
    args: &AttachArgs,
    config: &Config,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    let session = args
        .session
        .clone()
        .unwrap_or_else(|| config.session.default_name.clone());
    let prefix = config
        .keys
        .prefix_byte()
        .with_context(|| format!("prefix `{}` is not a ctrl+key chord", config.keys.prefix))?;
    let socket = SocketPath::for_session(&session)?;
    ensure_server(&session, &socket)?;
    let outcome = AttachApp::attach(&AttachOptions {
        socket,
        session: session.clone(),
        prefix,
        prefix_label: config.keys.prefix.clone(),
        window_title: config.ui.window_title.clone(),
    })?;
    writeln!(out, "{}", farewell(&session, outcome))?;
    Ok(ExitCode::SUCCESS)
}

/// The line printed after the client gives the terminal back.
fn farewell(session: &str, outcome: AttachOutcome) -> String {
    match outcome {
        AttachOutcome::Detached => format!("[detached from session {session}]"),
        AttachOutcome::ServerExited => format!("[session {session} ended]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farewell_names_the_session() {
        assert_eq!(
            farewell("main", AttachOutcome::Detached),
            "[detached from session main]"
        );
        assert_eq!(
            farewell("w", AttachOutcome::ServerExited),
            "[session w ended]"
        );
    }

    #[test]
    fn a_non_control_prefix_is_rejected_before_connecting() {
        let mut config = Config::default();
        config.keys.prefix = "alt+b".into();
        let err = run(&AttachArgs::default(), &config, &mut Vec::new()).expect_err("rejected");
        assert!(err.to_string().contains("alt+b"), "{err}");
    }
}
