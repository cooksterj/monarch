//! Monarch command-line interface: parses arguments and dispatches to a
//! command. Every command lives in its own file under `commands/`.

mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser;

pub use cli::{Cli, Command, ConfigCommand};

/// Parses `std::env::args` and runs the selected command.
#[must_use]
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout().lock();
    match commands::dispatch(cli, &mut stdout) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("monarch: {err:#}");
            ExitCode::FAILURE
        }
    }
}
