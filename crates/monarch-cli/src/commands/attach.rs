//! `monarch attach` (and a bare `monarch`): connect to a session.

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context;
use monarch_core::{Action, Config, Keymap};
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
    refuse_nesting(
        std::env::var("MONARCH_PANE").ok().as_deref(),
        std::env::var("MONARCH_SESSION").ok().as_deref(),
        config,
    )?;
    let prefix = config
        .keys
        .prefix_byte()
        .with_context(|| format!("prefix `{}` is not a ctrl+key chord", config.keys.prefix))?;
    let (keymap, warnings) = Keymap::from_config(&config.keys);
    let socket = SocketPath::for_session(&session)?;
    ensure_server(&session, &socket, &args.agents)?;
    let outcome = AttachApp::attach(&AttachOptions {
        socket,
        session: session.clone(),
        prefix,
        prefix_label: config.keys.prefix.clone(),
        window_title: config.ui.window_title.clone(),
        mouse: config.ui.mouse,
        sidebar: config.ui.sidebar,
        sidebar_width: config.ui.sidebar_width,
        sidebar_min_columns: config.ui.sidebar_min_columns,
        keymap,
        which_key_delay: Duration::from_millis(config.keys.which_key_delay_ms),
        repeat: Duration::from_millis(config.keys.repeat_ms),
        warnings,
    })?;
    writeln!(out, "{}", farewell(&session, outcome))?;
    Ok(ExitCode::SUCCESS)
}

/// Panes carry `MONARCH_PANE`. A client started inside one reads the very
/// pane it writes to, so every key loops back into it and none reach the
/// shell. Refuse, as tmux does; unsetting the variable deliberately nests.
fn refuse_nesting(
    pane: Option<&str>,
    session: Option<&str>,
    config: &Config,
) -> anyhow::Result<()> {
    if pane.is_none() {
        return Ok(());
    }
    let session = session.unwrap_or("?");
    let detach = match Keymap::from_config(&config.keys).0.key_for(Action::Detach) {
        Some(key) => format!("press {} {key} to detach", config.keys.prefix),
        None => "detach".to_owned(),
    };
    anyhow::bail!(
        "already inside monarch session `{session}`; {detach} instead \
         (to nest a session deliberately, run with MONARCH_PANE unset)"
    )
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
    fn attaching_from_inside_a_pane_is_refused() {
        let config = Config::default();
        let err = refuse_nesting(Some("1"), Some("main"), &config).expect_err("refused");
        let text = err.to_string();
        assert!(text.contains("inside monarch session `main`"), "{text}");
        assert!(text.contains("ctrl+b d"), "{text}");
        assert!(refuse_nesting(None, Some("main"), &config).is_ok());
        let mut herdr = Config::default();
        herdr.keys.preset = "herdr".into();
        let err = refuse_nesting(Some("1"), None, &herdr).expect_err("refused");
        assert!(err.to_string().contains("ctrl+b q"), "{err}");
    }

    #[test]
    fn a_non_control_prefix_is_rejected_before_connecting() {
        let mut config = Config::default();
        config.keys.prefix = "alt+b".into();
        let err = run(&AttachArgs::default(), &config, &mut Vec::new()).expect_err("rejected");
        assert!(err.to_string().contains("alt+b"), "{err}");
    }
}
