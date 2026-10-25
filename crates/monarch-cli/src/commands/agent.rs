//! `monarch agent explain`: why a pane's agent shows the state it does.

mod explain_text;

use std::io::Write;
use std::process::ExitCode;

use anyhow::bail;
use monarch_core::Config;
use monarch_proto::{ExplainPanes, PaneExplanation};
use monarch_server::SocketPath;

use crate::AgentCommand;
use crate::hook::ApiClient;

/// Asks the session's server and prints its answer.
pub(crate) fn run(
    cmd: &AgentCommand,
    config: &Config,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    let AgentCommand::Explain(args) = cmd;
    let session = super::session_name(args.session.clone(), config);
    let socket = SocketPath::for_session(&session)?;
    let Ok(mut client) = ApiClient::connect(&socket) else {
        bail!("no session named {session:?} is running (see `monarch ls`)");
    };
    let request = ExplainPanes { pane: args.pane };
    let explained: Vec<PaneExplanation> = match client.request(ExplainPanes::METHOD, &request) {
        Ok(explained) => explained,
        Err(e) if e.to_string().contains("unknown method") => {
            bail!("session {session:?} runs an older Monarch without `agent explain`")
        }
        Err(e) => return Err(e),
    };
    let text = match (args.pane, explained.as_slice()) {
        (Some(_), [one]) => explain_text::detail(one),
        _ => explain_text::list(&explained),
    };
    write!(out, "{text}")?;
    Ok(ExitCode::SUCCESS)
}
