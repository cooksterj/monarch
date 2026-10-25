//! `monarch ls`: every running session, its size and who is attached;
//! the same list feeds the session picker inside Monarch.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use monarch_proto::SessionInfo;
use monarch_server::SocketPath;
use monarch_tui::{SessionEntry, SessionLister};

use crate::hook::ApiClient;

/// Lists sessions by asking each one's socket.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SocketSessions;

impl SessionLister for SocketSessions {
    fn list(&self) -> Vec<SessionEntry> {
        sessions()
    }
}

/// Prints one line per running session.
pub(crate) fn run(out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let here = std::env::var("MONARCH_SESSION").ok();
    let sessions = sessions();
    if sessions.is_empty() {
        writeln!(out, "no sessions running")?;
        return Ok(ExitCode::SUCCESS);
    }
    let width = sessions
        .iter()
        .map(|s| s.name.chars().count())
        .max()
        .unwrap_or(0);
    for entry in &sessions {
        let here = here.as_deref() == Some(entry.name.as_str());
        writeln!(out, "{}", line(entry, width, here).trim_end())?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Every session socket somebody answers on (a server that died leaves
/// one nobody does), in name order.
fn sessions() -> Vec<SessionEntry> {
    SocketPath::all()
        .iter()
        .filter_map(|socket| {
            Some(SessionEntry {
                name: name(socket)?,
                info: info(socket)?,
            })
        })
        .collect()
}

fn name(socket: &Path) -> Option<String> {
    Some(socket.file_stem()?.to_string_lossy().into_owned())
}

/// `None` when nothing answers; `Some(None)` when a server answers but
/// doesn't know `session.info`.
fn info(socket: &Path) -> Option<Option<SessionInfo>> {
    let mut client = ApiClient::connect(socket).ok()?;
    Some(client.request(SessionInfo::METHOD, &()).ok())
}

/// `main   3 panes, 2 tabs    attached · here`.
fn line(entry: &SessionEntry, width: usize, here: bool) -> String {
    let name = &entry.name;
    let Some(info) = entry.info else {
        return format!("{name:width$}   (started by an older Monarch: no details)");
    };
    let attached = match info.clients {
        0 => String::new(),
        1 => "attached".to_owned(),
        n => format!("attached ×{n}"),
    };
    let notes: Vec<&str> = [attached.as_str(), if here { "here" } else { "" }]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    format!(
        "{name:width$}   {:16}   {}",
        entry.size(),
        notes.join(" · ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, panes: u32, tabs: u32, clients: u32) -> SessionEntry {
        SessionEntry {
            name: name.into(),
            info: Some(SessionInfo {
                panes,
                tabs,
                clients,
                pid: 1,
            }),
        }
    }

    #[test]
    fn lines_line_up_and_say_who_is_attached() {
        assert_eq!(
            line(&entry("main", 3, 2, 1), 4, true),
            "main   3 panes, 2 tabs    attached · here"
        );
        assert_eq!(
            line(&entry("w", 1, 1, 0), 4, false).trim_end(),
            "w      1 pane, 1 tab"
        );
        assert!(line(&entry("w", 1, 1, 2), 1, false).ends_with("attached ×2"));
        let old = SessionEntry {
            name: "old".into(),
            info: None,
        };
        assert!(line(&old, 3, false).contains("older Monarch"));
    }

    #[test]
    fn a_socket_is_named_by_its_file() {
        assert_eq!(
            name(Path::new("/run/monarch/work.sock")).as_deref(),
            Some("work")
        );
        assert_eq!(info(Path::new("/nonexistent/monarch.sock")), None, "dead");
    }
}
