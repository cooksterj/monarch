//! `monarch agent read`: print the last thing a pane's agent said.

use std::io::Write;
use std::process::ExitCode;

use monarch_core::PaneId;
use monarch_proto::{AgentRead, AgentReadReply, ReadSource};

use super::super::older;
use crate::ReadArgs;
use crate::hook::ApiClient;

/// Exit status when the agent has said nothing yet.
const NOTHING: u8 = 2;

/// Asks the server and prints the text; says so, with exit status 2,
/// when there is none.
pub(super) fn run(
    args: &ReadArgs,
    client: &mut ApiClient,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    let request = AgentRead { pane: args.pane };
    let reply: AgentReadReply = older(client.request(AgentRead::METHOD, &request), "agent read")?;
    write!(out, "{}", text(args.pane, &reply))?;
    Ok(if reply.source == ReadSource::Nothing {
        ExitCode::from(NOTHING)
    } else {
        ExitCode::SUCCESS
    })
}

/// The text with a final newline, or `p4 ○ Idle has said nothing yet`.
fn text(pane: PaneId, reply: &AgentReadReply) -> String {
    if reply.source == ReadSource::Nothing {
        let state = reply.status.state;
        return format!(
            "{pane} {} {} has said nothing yet\n",
            state.glyph(),
            state.label()
        );
    }
    format!("{}\n", reply.text.trim_end())
}

#[cfg(test)]
mod tests {
    use monarch_core::AgentState;
    use monarch_proto::AgentStatus;

    use super::*;

    #[test]
    fn prints_the_message_or_that_there_is_none() {
        let said = AgentReadReply {
            status: AgentStatus::default(),
            text: "Fixed it.\n\nTwo tests added.\n".into(),
            source: ReadSource::Message,
        };
        assert_eq!(text(PaneId(4), &said), "Fixed it.\n\nTwo tests added.\n");
        let nothing = AgentReadReply::nothing(AgentStatus {
            state: AgentState::Idle,
            ..AgentStatus::default()
        });
        assert_eq!(
            text(PaneId(4), &nothing),
            "p4 ○ Idle has said nothing yet\n"
        );
    }
}
