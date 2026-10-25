//! `monarch attach` (and a bare `monarch`): connect to a session.

use std::io::Write;
use std::path::{Path, PathBuf};
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
    let mut first = true;
    loop {
        let socket = SocketPath::for_session(&session)?;
        // Checked before a server is started for nothing.
        let mut opts = AttachOptions::new(socket.clone(), session.clone(), loaded)?;
        let unwelcomed = std::mem::take(&mut first)
            .then(StateFile::root)
            .and_then(|root| unwelcomed(&root.ok()?));
        opts.welcome = unwelcomed.is_some();
        ensure_server(&session, &socket, &args.agents)?;
        let outcome = AttachApp::attach(&opts, Arc::clone(&lister))?;
        if let Some(marker) = unwelcomed {
            remember(&marker);
        }
        match outcome {
            AttachOutcome::Switch(next) => session = next,
            outcome => {
                writeln!(out, "{}", farewell(&session, &outcome))?;
                break;
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// The marker to write after the first attach on this machine, when
/// Monarch's state directory `root` doesn't have it yet. Nothing is
/// written here: an attach that fails must not use up the welcome.
fn unwelcomed(root: &Path) -> Option<PathBuf> {
    let marker = root.join("welcomed");
    (!marker.exists()).then_some(marker)
}

/// Writes `marker` once the welcome has been shown. If that fails the
/// welcome shows again next time, which beats never showing it.
fn remember(marker: &Path) {
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(marker, "");
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
        let marker = unwelcomed(&root).expect("first attach");
        assert!(!root.exists(), "nothing written before the attach");
        remember(&marker);
        assert_eq!(unwelcomed(&root), None, "never again");
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
