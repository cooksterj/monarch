//! `monarch attach` (and a bare `monarch`): connect to a session.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use monarch_core::{Action, Config, Keymap, LoadedConfig};
use monarch_server::{SocketPath, StateFile};
use monarch_tui::{AttachApp, AttachOptions, AttachOutcome, SessionLister};

use super::auto_start::ensure_server;
use super::ls::SocketSessions;
use crate::AttachArgs;

/// Attaches, starting the server first if it is not running.
pub(crate) fn run(
    args: &AttachArgs,
    loaded: &LoadedConfig,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    let config = &loaded.config;
    let mut session = args
        .session
        .clone()
        .unwrap_or_else(|| config.session.default_name.clone());
    refuse_nesting(
        std::env::var("MONARCH_PANE").ok().as_deref(),
        std::env::var("MONARCH_SESSION").ok().as_deref(),
        config,
    )?;
    let lister: Arc<dyn SessionLister> = Arc::new(SocketSessions);
    let mut welcome = StateFile::root().is_ok_and(|root| first_run(&root));
    loop {
        let socket = SocketPath::for_session(&session)?;
        // Checked before a server is started for nothing.
        let mut opts = AttachOptions::new(socket.clone(), session.clone(), loaded)?;
        opts.welcome = std::mem::take(&mut welcome);
        ensure_server(&session, &socket, &args.agents)?;
        match AttachApp::attach(&opts, Arc::clone(&lister))? {
            AttachOutcome::Switch(next) => session = next,
            outcome => {
                writeln!(out, "{}", farewell(&session, &outcome))?;
                break;
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Whether this is the first attach on this machine, judged by a marker
/// in Monarch's state directory, which it then writes. Any error counts
/// as not first: a welcome every time would be worse than none.
fn first_run(root: &Path) -> bool {
    let marker = root.join("welcomed");
    if marker.exists() {
        return false;
    }
    std::fs::create_dir_all(root).is_ok() && std::fs::write(&marker, "").is_ok()
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
fn farewell(session: &str, outcome: &AttachOutcome) -> String {
    match outcome {
        AttachOutcome::Detached => format!("[detached from session {session}]"),
        AttachOutcome::ServerExited => format!("[session {session} ended]"),
        AttachOutcome::Switch(next) => format!("[switched from session {session} to {next}]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_welcome_shows_once() {
        let dir = tempfile::tempdir().expect("dir");
        let root = dir.path().join("state/monarch");
        assert!(first_run(&root), "first attach");
        assert!(!first_run(&root), "never again");
        let file = dir.path().join("file");
        std::fs::write(&file, "").expect("write");
        assert!(
            !first_run(&file.join("monarch")),
            "an unwritable root shows nothing"
        );
    }

    #[test]
    fn farewell_names_the_session() {
        assert_eq!(
            farewell("main", &AttachOutcome::Detached),
            "[detached from session main]"
        );
        assert_eq!(
            farewell("w", &AttachOutcome::ServerExited),
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
        let loaded = LoadedConfig {
            config,
            path: "/c.toml".into(),
            found: false,
            unknown_keys: Vec::new(),
        };
        let err = run(&AttachArgs::default(), &loaded, &mut Vec::new()).expect_err("rejected");
        assert!(err.to_string().contains("alt+b"), "{err}");
    }
}
