//! Everything `monarch doctor` looks at, gathered once, so the checks are
//! pure functions of it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use monarch_server::SocketPath;
use monarch_tui::SessionLister;

use super::super::ls::SocketSessions;
use super::super::setup::SetupEnv;

/// Environment variables the terminal check reads.
pub(crate) const VARS: [&str; 5] = [
    "TERM",
    "COLORTERM",
    "TERM_PROGRAM",
    "NO_COLOR",
    "MONARCH_PANE",
];

/// The machine as doctor sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DoctorEnv {
    /// Where the agents' settings are and what hooks should run.
    pub setup: SetupEnv,
    /// The values of [`VARS`] that are set.
    pub vars: BTreeMap<&'static str, String>,
    /// `PATH`'s directories, in order.
    pub path: Vec<PathBuf>,
    /// `git --version`, when git runs.
    pub git: Option<String>,
    /// Running sessions' names.
    pub sessions: Vec<String>,
    /// The running sessions whose server started before this binary was
    /// installed, so they still run the build before it.
    pub outdated: Vec<String>,
}

impl DoctorEnv {
    /// Reads the environment, runs `git --version` and asks every session
    /// socket.
    pub fn from_env() -> Self {
        let vars = VARS
            .into_iter()
            .filter_map(|v| Some((v, std::env::var(v).ok()?)))
            .collect();
        let path = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        let git = Command::new("git")
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
        let sessions: Vec<String> = SocketSessions.list().into_iter().map(|s| s.name).collect();
        let installed = std::env::current_exe().and_then(|p| p.metadata()?.modified());
        let outdated = installed.map_or_else(|_| Vec::new(), |at| older(&sessions, at));
        Self {
            setup: SetupEnv::from_env(),
            vars,
            path,
            git,
            sessions,
            outdated,
        }
    }

    /// The variable's value when set and not empty.
    pub fn var(&self, name: &str) -> Option<&str> {
        self.vars
            .get(name)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    /// Where `program` is on `PATH`, if anywhere.
    pub fn which(&self, program: &str) -> Option<PathBuf> {
        self.path
            .iter()
            .map(|d| d.join(program))
            .find(|p| p.is_file())
    }
}

/// Which of `sessions` had their socket made before `installed`: a server
/// makes its socket when it starts, and never touches it again.
fn older(sessions: &[String], installed: SystemTime) -> Vec<String> {
    let started = |socket: &std::path::Path| socket.metadata().and_then(|m| m.modified()).ok();
    SocketPath::all()
        .iter()
        .filter(|socket| started(socket).is_some_and(|at| at < installed))
        .filter_map(|socket| Some(socket.file_stem()?.to_string_lossy().into_owned()))
        .filter(|name| sessions.contains(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_are_found_on_path_in_order() {
        let a = tempfile::tempdir().expect("dir");
        let b = tempfile::tempdir().expect("dir");
        std::fs::write(b.path().join("claude"), "").expect("write");
        let env = DoctorEnv {
            setup: SetupEnv {
                claude: a.path().join(".claude"),
                codex: a.path().join(".codex"),
                agents: a.path().join(".agents"),
                program: "monarch".into(),
                stamp: 0,
            },
            vars: BTreeMap::from([("TERM", String::new())]),
            path: vec![a.path().into(), b.path().into()],
            git: None,
            sessions: Vec::new(),
            outdated: Vec::new(),
        };
        assert_eq!(env.which("claude"), Some(b.path().join("claude")));
        assert_eq!(env.which("codex"), None);
        assert_eq!(env.var("TERM"), None, "empty counts as unset");
    }
}
