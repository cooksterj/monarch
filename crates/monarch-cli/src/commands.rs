//! Routes a parsed [`Cli`] to the function that implements it.
//!
//! Commands write to an injected `Write` so they can be tested without
//! capturing the process's stdout.

mod config;
mod welcome;

use std::io::Write;
use std::process::ExitCode;

use monarch_core::ConfigLoader;

use crate::{Cli, Command};

/// Runs the command selected by `cli`, writing user-facing output to `out`.
pub(crate) fn dispatch(cli: Cli, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let loader = ConfigLoader::from_env();
    match cli.command {
        None => welcome::run(&loader, out),
        Some(Command::Config(cmd)) => config::run(&cmd, &loader, out),
    }
}
