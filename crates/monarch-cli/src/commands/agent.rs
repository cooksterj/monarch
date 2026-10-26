//! `monarch agent …`: ask a session about its agents: why a pane shows
//! its state (`explain`), or wait for one to reach a state (`wait`).

mod explain_text;
mod wait;

use std::io::Write;
use std::process::ExitCode;

use monarch_core::Config;
use monarch_proto::{ExplainPanes, PaneExplanation};

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
            wait::run(args, &mut super::connect(&session)?, out)
        }
    }
}

/// Asks the session's server why and prints its answer.
fn explain(args: &ExplainArgs, config: &Config, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let session = super::session_name(args.session.clone(), config);
    let mut client = super::connect(&session)?;
    let request = ExplainPanes { pane: args.pane };
    let explained: Vec<PaneExplanation> = super::older(
        client.request(ExplainPanes::METHOD, &request),
        "agent explain",
    )?;
    let text = match (args.pane, explained.as_slice()) {
        (Some(_), [one]) => explain_text::detail(one),
        _ => explain_text::list(&explained),
    };
    write!(out, "{text}")?;
    Ok(ExitCode::SUCCESS)
}
