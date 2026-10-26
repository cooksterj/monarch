//! Checks of the machine Monarch runs on: the binary on `PATH`, git, the
//! agents, the terminal and running sessions.

use std::path::Path;

use super::{DoctorEnv, Finding, Status};
use crate::HookCommand;

/// Whether typing `monarch` runs this binary.
pub(super) fn binary(env: &DoctorEnv, exe: &Path) -> Finding {
    if env.setup.program == Path::new("monarch") {
        return Finding::new(
            Status::Ok,
            "monarch",
            format!("{} is on PATH", exe.display()),
        );
    }
    let dir = exe.parent().unwrap_or(exe).display();
    let text = match env.which("monarch") {
        Some(other) => format!("`monarch` on PATH is {}, not this one", other.display()),
        None => "`monarch` is not on PATH".to_owned(),
    };
    Finding::new(Status::Warn, "monarch", text).fix(format!(
        "add {dir} to PATH, or `cargo install --path .` (hooks use the full path either way)"
    ))
}

/// git: worktrees, review and the sidebar's badges run it.
pub(super) fn git(env: &DoctorEnv) -> Finding {
    match &env.git {
        Some(version) => Finding::new(Status::Ok, "git", version.clone()),
        None => Finding::new(
            Status::Fail,
            "git",
            "not found: worktrees, review and badges need it",
        )
        .fix("install git"),
    }
}

/// Which agents are on `PATH`.
pub(super) fn agents(env: &DoctorEnv) -> Finding {
    let (found, missing): (Vec<_>, Vec<_>) = HookCommand::ALL
        .into_iter()
        .partition(|agent| env.which(agent.name()).is_some());
    let names =
        |list: Vec<HookCommand>| list.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ");
    match (found.is_empty(), missing.is_empty()) {
        (true, _) => Finding::new(
            Status::Note,
            "agents",
            "neither claude nor codex is on PATH",
        ),
        (false, true) => Finding::new(Status::Ok, "agents", format!("{} on PATH", names(found))),
        (false, false) => Finding::new(
            Status::Ok,
            "agents",
            format!("{} on PATH ({} not found)", names(found), names(missing)),
        ),
    }
}

/// The terminal this runs in: its type, its name, and whether it is a
/// Monarch pane.
pub(super) fn terminal(env: &DoctorEnv) -> Vec<Finding> {
    let mut found = Vec::new();
    if env.var("MONARCH_PANE").is_some() {
        found.push(Finding::new(
            Status::Note,
            "terminal",
            "this is a Monarch pane; the checks below describe it, not your terminal",
        ));
    }
    match env.var("TERM") {
        None | Some("dumb") => found.push(
            Finding::new(Status::Warn, "terminal", "TERM is unset or dumb")
                .fix("run Monarch in a full terminal"),
        ),
        Some(term) => found.push(Finding::new(Status::Ok, "terminal", format!("TERM={term}"))),
    }
    if let Some(host) = env.var("TERM_PROGRAM") {
        found.push(Finding::new(
            Status::Note,
            "terminal",
            format!("running in {host}"),
        ));
    }
    found
}

/// Whether the terminal shows the 24-bit colours themes use.
pub(super) fn colour(env: &DoctorEnv) -> Vec<Finding> {
    let mut found = Vec::new();
    match env.var("COLORTERM") {
        Some("truecolor" | "24bit") => {
            found.push(Finding::new(
                Status::Ok,
                "colour",
                "24-bit colour (COLORTERM)",
            ));
        }
        other => found.push(
            Finding::new(
                Status::Warn,
                "colour",
                format!(
                    "COLORTERM is {}: themes use 24-bit colour",
                    other.unwrap_or("unset")
                ),
            )
            .fix("use a terminal with true colour, or set COLORTERM=truecolor if yours has it"),
        ),
    }
    if env.var("NO_COLOR").is_some() {
        found.push(Finding::new(
            Status::Note,
            "colour",
            "NO_COLOR is set: Monarch draws without colours",
        ));
    }
    found
}

/// Running sessions.
pub(super) fn sessions(env: &DoctorEnv) -> Finding {
    match env.sessions.len() {
        0 => Finding::new(
            Status::Note,
            "sessions",
            "none running; `monarch` starts one",
        ),
        n => Finding::new(
            Status::Ok,
            "sessions",
            format!("{n} running: {}", env.sessions.join(", ")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::commands::setup::SetupEnv;

    fn env(vars: &[(&'static str, &str)]) -> DoctorEnv {
        DoctorEnv {
            setup: SetupEnv {
                claude: "/h/.claude".into(),
                codex: "/h/.codex".into(),
                agents: "/h/.agents".into(),
                program: "/opt/bin/monarch".into(),
                stamp: 0,
            },
            vars: vars
                .iter()
                .map(|(k, v)| (*k, (*v).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            path: Vec::new(),
            git: None,
            sessions: Vec::new(),
        }
    }

    #[test]
    fn terminals_without_true_colour_are_warned() {
        let both = |e: &DoctorEnv| [terminal(e), colour(e)].concat();
        let good = both(&env(&[
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
        ]));
        assert!(good.iter().all(|f| f.status == Status::Ok), "{good:?}");
        let plain = both(&env(&[
            ("TERM", "xterm"),
            ("NO_COLOR", "1"),
            ("MONARCH_PANE", "3"),
        ]));
        assert_eq!(plain[0].status, Status::Note, "inside a pane comes first");
        let warned = |f: &Finding| f.status == Status::Warn && f.topic == "colour";
        assert!(plain.iter().any(warned), "{plain:?}");
        assert!(plain.iter().any(|f| f.text.starts_with("NO_COLOR")));
        assert_eq!(terminal(&env(&[]))[0].status, Status::Warn, "no TERM");
    }

    #[test]
    fn missing_tools_and_paths_are_reported() {
        let e = env(&[]);
        assert_eq!(git(&e).status, Status::Fail);
        assert_eq!(agents(&e).status, Status::Note);
        let off_path = binary(&e, Path::new("/opt/bin/monarch"));
        assert_eq!(off_path.status, Status::Warn);
        assert!(off_path.fix.is_some_and(|f| f.contains("/opt/bin")));
        let on_path = DoctorEnv {
            setup: SetupEnv {
                program: "monarch".into(),
                ..e.setup.clone()
            },
            ..e.clone()
        };
        assert_eq!(binary(&on_path, Path::new("/x/monarch")).status, Status::Ok);
        let running = DoctorEnv {
            sessions: vec!["main".into()],
            ..e
        };
        assert_eq!(sessions(&running).text, "1 running: main");
    }
}
