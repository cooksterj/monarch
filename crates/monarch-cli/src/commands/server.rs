//! `monarch server`: run a session server.

use std::process::ExitCode;

use monarch_core::Config;
use monarch_server::{Daemon, LaunchConfig, PaneSpec, Server, ServerConfig, StateFile};

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
        ..PaneSpec::default()
    };
    let mut panes: Vec<_> = args
        .panes
        .iter()
        .map(|cmd| PaneSpec::shell_command(cmd))
        .chain(
            args.agents
                .iter()
                .map(|kind| PaneSpec::agent(*kind, &config.agent(*kind))),
        )
        .collect();
    let fresh = !panes.is_empty();
    if !fresh {
        panes.push(shell);
    }
    let session = args
        .session
        .clone()
        .unwrap_or_else(|| config.session.default_name.clone());
    ServerConfig {
        state: StateFile::for_session(&session).ok(),
        // Panes named on the command line start a new session; otherwise
        // a session that was killed comes back as it was.
        restore: !fresh,
        session,
        socket: None,
        panes,
        launch: LaunchConfig::from(config),
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentKind;

    use super::*;

    #[test]
    fn defaults_come_from_config() {
        let mut config = Config::default();
        config.terminal.scrollback = 42;
        config.session.resume_agents = false;
        let server = server_config(&ServerArgs::default(), &config);
        assert_eq!(server.session, "main");
        assert_eq!(server.launch.scrollback, 42);
        assert!(!server.launch.resume_agents);
        assert_eq!(server.panes, [PaneSpec::default()]);
        assert!(server.restore);
    }

    #[test]
    fn explicit_panes_and_session_win() {
        let args = ServerArgs {
            session: Some("w".into()),
            panes: vec!["htop".into()],
            ..ServerArgs::default()
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

    #[test]
    fn agents_start_a_fresh_session_after_any_commands() {
        let args = ServerArgs {
            panes: vec!["htop".into()],
            agents: vec![AgentKind::Claude],
            ..ServerArgs::default()
        };
        let server = server_config(&args, &Config::default());
        assert!(!server.restore);
        assert_eq!(server.panes.len(), 2);
        assert_eq!(server.panes[1].argv[..2], ["claude", "--session-id"]);
    }

    #[test]
    fn agents_launch_as_the_config_says() {
        let mut config = Config::default();
        let claude = config.agents.get_mut("claude").expect("claude");
        claude.command = "my-claude".into();
        claude.args = vec!["--model".into(), "opus".into()];
        let args = ServerArgs {
            agents: vec![AgentKind::Claude],
            ..ServerArgs::default()
        };
        let server = server_config(&args, &config);
        assert_eq!(server.panes[0].argv, ["my-claude", "--model", "opus"]);
        assert_eq!(
            server.panes[0]
                .agent
                .as_ref()
                .and_then(|a| a.session_id.clone()),
            None,
            "no session id placeholder in args: the agent picks its own"
        );
    }
}
