//! `monarch agent …`: ask a session about its agents: why a pane shows
//! its state (`explain`), or wait for one to reach a state (`wait`).

mod explain_text;
mod wait;

use std::io::Write;
use std::process::ExitCode;

use anyhow::bail;
use monarch_core::Config;
use monarch_proto::{ExplainPanes, PaneExplanation};
use monarch_server::SocketPath;

use crate::hook::ApiClient;
use crate::{AgentCommand, ExplainArgs};

/// Runs one `monarch agent` command.
pub(crate) fn run(
    cmd: &AgentCommand,
    config: &Config,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    match cmd {
        AgentCommand::Explain(args) => explain(args, config, out),
        AgentCommand::Wait(args) => {
            let session = super::session_name(args.session.clone(), config);
            wait::run(args, &mut connect(&session)?, out)
        }
    }
}

/// Asks the session's server why and prints its answer.
fn explain(args: &ExplainArgs, config: &Config, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let session = super::session_name(args.session.clone(), config);
    let mut client = connect(&session)?;
    let request = ExplainPanes { pane: args.pane };
    let explained: Vec<PaneExplanation> =
        older(client.request(ExplainPanes::METHOD, &request), "explain")?;
    let text = match (args.pane, explained.as_slice()) {
        (Some(_), [one]) => explain_text::detail(one),
        _ => explain_text::list(&explained),
    };
    write!(out, "{text}")?;
    Ok(ExitCode::SUCCESS)
}

/// A client for `session`'s running server.
fn connect(session: &str) -> anyhow::Result<ApiClient> {
    let socket = SocketPath::for_session(session)?;
    match ApiClient::connect(&socket) {
        Ok(client) => Ok(client),
        Err(_) => bail!("no session named {session:?} is running (see `monarch ls`)"),
    }
}

/// `result`, with a server too old to know the method named as such.
fn older<T>(result: anyhow::Result<T>, verb: &str) -> anyhow::Result<T> {
    match result {
        Err(e) if e.to_string().contains("unknown method") => {
            bail!("that session runs an older Monarch without `agent {verb}`")
        }
        other => other,
    }
}
