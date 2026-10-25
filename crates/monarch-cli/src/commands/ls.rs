//! `monarch ls`: every running session, its size and who is attached.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use monarch_proto::SessionInfo;
use monarch_server::SocketPath;

use crate::hook::ApiClient;

/// Asks each session socket for its info; sockets nobody answers on
/// (a server that died) are left out, and a server too old to say is
/// listed without details.
pub(crate) fn run(out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let here = std::env::var("MONARCH_SESSION").ok();
    let sessions: Vec<(String, Option<SessionInfo>)> = SocketPath::all()
        .iter()
        .filter_map(|socket| Some((name(socket)?, info(socket)?)))
        .collect();
    if sessions.is_empty() {
        writeln!(out, "no sessions running")?;
        return Ok(ExitCode::SUCCESS);
    }
    let width = sessions
        .iter()
        .map(|(n, _)| n.chars().count())
        .max()
        .unwrap_or(0);
    for (name, info) in &sessions {
        let here = here.as_deref() == Some(name.as_str());
        let text = match info {
            Some(info) => line(name, width, info, here),
            None => format!("{name:width$}   (started by an older Monarch: no details)"),
        };
        writeln!(out, "{}", text.trim_end())?;
    }
    Ok(ExitCode::SUCCESS)
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

/// `main   3 panes, 2 tabs   attached · here`.
fn line(name: &str, width: usize, info: &SessionInfo, here: bool) -> String {
    let plural = |n: u32, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    let size = format!(
        "{}, {}",
        plural(info.panes, "pane"),
        plural(info.tabs, "tab")
    );
    let attached = match info.clients {
        0 => String::new(),
        1 => "attached".to_owned(),
        n => format!("attached ×{n}"),
    };
    let notes: Vec<&str> = [attached.as_str(), if here { "here" } else { "" }]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    format!("{name:width$}   {size:16}   {}", notes.join(" · "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(panes: u32, tabs: u32, clients: u32) -> SessionInfo {
        SessionInfo {
            panes,
            tabs,
            clients,
            pid: 1,
        }
    }

    #[test]
    fn lines_line_up_and_say_who_is_attached() {
        assert_eq!(
            line("main", 4, &info(3, 2, 1), true),
            "main   3 panes, 2 tabs    attached · here"
        );
        assert_eq!(
            line("w", 4, &info(1, 1, 0), false).trim_end(),
            "w      1 pane, 1 tab"
        );
        assert!(line("w", 1, &info(1, 1, 2), false).ends_with("attached ×2"));
    }

    #[test]
    fn a_socket_is_named_by_its_file() {
        assert_eq!(
            name(Path::new("/run/monarch/work.sock")).as_deref(),
            Some("work")
        );
        assert_eq!(
            super::info(Path::new("/nonexistent/monarch.sock")),
            None,
            "dead"
        );
    }
}
