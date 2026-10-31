//! `monarch tab …`: act on a session's tabs; for now, name one
//! (`rename`).

use std::io::Write;
use std::process::ExitCode;

use monarch_core::{Config, PaneId};
use monarch_proto::{TabRename, TabRenamed};

use crate::{RenameArgs, TabCommand};

/// Runs one `monarch tab` command.
pub(crate) fn run(
    cmd: &TabCommand,
    config: &Config,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    match cmd {
        TabCommand::Rename(args) => rename(args, config, out),
    }
}

/// Names the tab and says what it is called now.
fn rename(args: &RenameArgs, config: &Config, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let session = super::session_name(args.session.clone(), config);
    let inside = std::env::var("MONARCH_SESSION").ok();
    let pane = std::env::var("MONARCH_PANE").ok();
    let request = request(args, &session, inside.as_deref(), pane.as_deref());
    let mut client = super::connect(&session)?;
    let renamed: TabRenamed =
        super::older(client.request(TabRename::METHOD, &request), "tab rename")?;
    writeln!(out, "{}", said(&renamed))?;
    Ok(ExitCode::SUCCESS)
}

/// The request: the tab asked for, else the one holding this pane when
/// it runs inside `session`, else (no target) the one shown.
fn request(
    args: &RenameArgs,
    session: &str,
    inside: Option<&str>,
    pane: Option<&str>,
) -> TabRename {
    let own = (inside == Some(session))
        .then(|| pane.and_then(|p| p.parse::<PaneId>().ok()))
        .flatten();
    TabRename {
        tab: args.tab.map(usize::from),
        pane: if args.tab.is_some() { None } else { own },
        name: args.name.clone(),
    }
}

/// `tab 2 is now "api"`, or that it is back to its automatic name.
fn said(renamed: &TabRenamed) -> String {
    match &renamed.name {
        Some(name) => format!("tab {} is now {name:?}", renamed.tab),
        None => format!("tab {} has its automatic name again", renamed.tab),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(tab: Option<u16>) -> RenameArgs {
        RenameArgs {
            name: "api".into(),
            tab,
            session: None,
        }
    }

    #[test]
    fn your_own_tab_unless_one_is_named_or_you_are_elsewhere() {
        let own = request(&args(None), "main", Some("main"), Some("p3"));
        assert_eq!((own.tab, own.pane), (None, Some(PaneId(3))));
        let numbered = request(&args(Some(2)), "main", Some("main"), Some("p3"));
        assert_eq!((numbered.tab, numbered.pane), (Some(2), None));
        let other = request(&args(None), "work", Some("main"), Some("p3"));
        assert_eq!(
            (other.tab, other.pane),
            (None, None),
            "p3 is another session's"
        );
        let outside = request(&args(None), "main", None, None);
        assert_eq!((outside.tab, outside.pane), (None, None), "the shown tab");
        let odd = request(&args(None), "main", Some("main"), Some("nonsense"));
        assert_eq!(odd.pane, None);
    }

    #[test]
    fn says_what_the_tab_is_called_now() {
        let named = TabRenamed {
            tab: 2,
            name: Some("api".into()),
        };
        assert_eq!(said(&named), r#"tab 2 is now "api""#);
        let cleared = TabRenamed { tab: 1, name: None };
        assert_eq!(said(&cleared), "tab 1 has its automatic name again");
    }
}
