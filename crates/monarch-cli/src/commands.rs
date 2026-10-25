//! Routes a parsed [`Cli`] to the function that implements it.
//!
//! Commands write to an injected `Write` so they can be tested without
//! capturing the process's stdout.

mod attach;
mod auto_start;
mod config;
mod keys;
mod server;

use std::io::Write;
use std::process::ExitCode;

use monarch_core::ConfigLoader;

use crate::{AttachArgs, Cli, Command};

/// Runs the command selected by `cli`, writing user-facing output to `out`.
pub(crate) fn dispatch(cli: Cli, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let loader = ConfigLoader::from_env();
    match cli.command {
        Some(Command::Config(cmd)) => config::run(&cmd, &loader, out),
        Some(Command::Keys) => keys::run(&loader.load()?.config, out),
        Some(Command::Server(args)) => server::run(&args, &loader.load()?.config),
        Some(Command::Attach(args)) => attach::run(&args, &loader.load()?, out),
        None => attach::run(&AttachArgs::default(), &loader.load()?, out),
    }
}
