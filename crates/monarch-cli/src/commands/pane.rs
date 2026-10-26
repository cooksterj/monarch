//! `monarch pane …`: act on a session's panes; for now, type into one
//! (`send`).

use std::io::{Read, Write};
use std::process::ExitCode;

use anyhow::{Context, bail};
use monarch_core::{Config, PaneId};
use monarch_proto::PaneSend;

use crate::{PaneCommand, SendArgs};

/// Runs one `monarch pane` command; `input` is standard input.
pub(crate) fn run(
    cmd: &PaneCommand,
    config: &Config,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    match cmd {
        PaneCommand::Send(args) => send(args, config, input, out),
    }
}

/// Types the text into the pane, refusing the pane this runs in.
fn send(
    args: &SendArgs,
    config: &Config,
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    let own = std::env::var("MONARCH_PANE").ok();
    refuse_own_pane(args.pane, own.as_deref())?;
    let text = match &args.text {
        Some(text) => text.clone(),
        None => {
            let mut text = String::new();
            input
                .read_to_string(&mut text)
                .context("reading the text from stdin")?;
            text
        }
    };
    let session = super::session_name(args.session.clone(), config);
    let mut client = super::connect(&session)?;
    let request = PaneSend {
        pane: args.pane,
        text,
        enter: args.enter,
    };
    let _: serde_json::Value =
        super::older(client.request(PaneSend::METHOD, &request), "pane send")?;
    out.flush()?;
    Ok(ExitCode::SUCCESS)
}

/// Typing into your own pane would feed your own input back to you.
fn refuse_own_pane(pane: PaneId, own: Option<&str>) -> anyhow::Result<()> {
    if own.and_then(|o| o.parse::<PaneId>().ok()) == Some(pane) {
        bail!("{pane} is the pane this runs in; send to another pane");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn your_own_pane_is_refused() {
        let err = refuse_own_pane(PaneId(3), Some("p3")).expect_err("refused");
        assert!(
            err.to_string().contains("p3 is the pane this runs in"),
            "{err}"
        );
        assert!(refuse_own_pane(PaneId(3), Some("p4")).is_ok());
        assert!(refuse_own_pane(PaneId(3), None).is_ok(), "outside Monarch");
    }

    #[test]
    fn a_session_that_isnt_running_is_named() {
        let args = SendArgs {
            pane: PaneId(4),
            text: Some("hi".into()),
            enter: true,
            session: Some("no-such-session-for-tests".into()),
        };
        let err =
            send(&args, &Config::default(), &mut &b""[..], &mut Vec::new()).expect_err("no server");
        assert!(
            err.to_string().contains("no-such-session-for-tests"),
            "{err}"
        );
    }
}
