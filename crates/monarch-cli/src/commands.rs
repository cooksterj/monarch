//! Routes a parsed [`Cli`] to the function that implements it.
//!
//! Commands write to an injected `Write` so they can be tested without
//! capturing the process's stdout.

mod agent;
mod attach;
mod auto_start;
mod config;
mod doctor;
mod hook;
mod keys;
mod kill;
mod ls;
mod new;
mod pane;
mod server;
mod setup;

use std::io::Write;
use std::process::ExitCode;

use anyhow::bail;
use monarch_core::{Config, ConfigLoader};
use monarch_server::SocketPath;

use crate::hook::ApiClient;

use crate::{AttachArgs, Cli, Command};

/// Runs the command selected by `cli`, writing user-facing output to `out`.
pub(crate) fn dispatch(cli: Cli, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    if let Some(Command::Hook(cmd)) = cli.command {
        return Ok(hook::run(
            cmd,
            &hook::HookEnv::from_env(),
            std::io::stdin().lock(),
        ));
    }
    let loader = ConfigLoader::from_env();
    match cli.command {
        Some(Command::Hook(_)) => Ok(ExitCode::SUCCESS),
        Some(Command::Config(cmd)) => config::run(&cmd, &loader, out),
        Some(Command::Doctor) => {
            let loaded = loader.load();
            let exe = std::env::current_exe().unwrap_or_default();
            let env = doctor::DoctorEnv::from_env();
            let found = doctor::findings(&env, loaded.as_ref().map_err(ToString::to_string), &exe);
            doctor::run(&found, out)
        }
        Some(Command::Setup(args)) => setup::run(args, &setup::SetupEnv::from_env(), out),
        Some(Command::Ls) => ls::run(out),
        Some(Command::Kill(args)) => kill::run(&args, out),
        Some(Command::Agent(cmd)) => agent::run(&cmd, &loader.load()?.config, out),
        Some(Command::Pane(cmd)) => pane::run(
            &cmd,
            &loader.load()?.config,
            &mut std::io::stdin().lock(),
            out,
        ),
        Some(Command::Keys) => keys::run(&loader.load()?.config, out),
        Some(Command::Server(args)) => server::run(&args, &loader.load()?.config),
        Some(Command::New(args)) => new::run(&args, &loader.load()?, out),
        Some(Command::Attach(args)) => attach::run(&args, &loader.load()?, out),
        None => attach::run(&AttachArgs::default(), &loader.load()?, out),
    }
}

/// A client for `session`'s running server.
pub(crate) fn connect(session: &str) -> anyhow::Result<ApiClient> {
    let socket = SocketPath::for_session(session)?;
    match ApiClient::connect(&socket) {
        Ok(client) => Ok(client),
        Err(_) => bail!("no session named {session:?} is running (see `monarch ls`)"),
    }
}

/// `result`, with a server too old to know the method (`command`, as
/// typed) named as such.
pub(crate) fn older<T>(result: anyhow::Result<T>, command: &str) -> anyhow::Result<T> {
    match result {
        Err(e) if e.to_string().contains("unknown method") => {
            bail!("that session runs an older Monarch without `monarch {command}`")
        }
        other => other,
    }
}

/// The session a command means: the one named, else the one it runs
/// inside, else the configured default.
pub(crate) fn session_name(named: Option<String>, config: &Config) -> String {
    let inside = std::env::var("MONARCH_PANE")
        .is_ok()
        .then(|| std::env::var("MONARCH_SESSION").ok())
        .flatten();
    named
        .or(inside)
        .unwrap_or_else(|| config.session.default_name.clone())
}
