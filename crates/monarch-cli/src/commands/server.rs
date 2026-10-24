//! `monarch server`: run a session server.

use std::process::ExitCode;

use monarch_core::Config;
use monarch_server::{Daemon, PaneSpec, Server, ServerConfig, StateFile};

use crate::ServerArgs;

/// Runs the server until its last pane exits.
pub(crate) fn run(args: &ServerArgs, config: &Config) -> anyhow::Result<ExitCode> {
    if args.daemon {
        Daemon::detach()?;
    }
    Server::run(server_config(args, config))?;
    Ok(ExitCode::SUCCESS)
}

fn server_config(args: &ServerArgs, config: &Config) -> ServerConfig {
    let shell = PaneSpec {
        argv: config.terminal.shell.iter().cloned().collect(),
        cwd: None,
    };
    let panes = if args.panes.is_empty() {
        vec![shell]
    } else {
        args.panes
            .iter()
            .map(|cmd| PaneSpec::shell_command(cmd))
            .collect()
    };
    let session = args
        .session
        .clone()
        .unwrap_or_else(|| config.session.default_name.clone());
    ServerConfig {
        state: StateFile::for_session(&session).ok(),
        // Panes named on the command line start a new session; otherwise
        // a session that was killed comes back as it was.
        restore: args.panes.is_empty(),
        session,
        socket: None,
        panes,
        scrollback: config.terminal.scrollback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_come_from_config() {
        let mut config = Config::default();
        config.terminal.scrollback = 42;
        let server = server_config(&ServerArgs::default(), &config);
        assert_eq!(server.session, "main");
        assert_eq!(server.scrollback, 42);
        assert_eq!(server.panes, [PaneSpec::default()]);
        assert!(server.restore);
    }

    #[test]
    fn explicit_panes_and_session_win() {
        let args = ServerArgs {
            session: Some("w".into()),
            panes: vec!["htop".into()],
            daemon: false,
        };
        let server = server_config(&args, &Config::default());
        assert_eq!(server.session, "w");
        assert_eq!(server.panes, [PaneSpec::shell_command("htop")]);
        assert!(!server.restore, "explicit panes start fresh");
        assert!(
            server
                .state
                .is_some_and(|s| s.path().ends_with("w/state.json"))
        );
    }
}
