//! What to run in a new pane and the environment it gets.

use std::path::PathBuf;

use monarch_core::PaneId;
use portable_pty::CommandBuilder;

use crate::PaneEnv;

/// Variables from enclosing multiplexers that would mislead programs.
const FOREIGN_VARS: [&str; 5] = ["TMUX", "TMUX_PANE", "STY", "ZELLIJ", "ZELLIJ_SESSION_NAME"];

/// A program to start in a pane. An empty `argv` means the user's shell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneSpec {
    /// Program and arguments.
    pub argv: Vec<String>,
    /// Working directory; the server's when `None`.
    pub cwd: Option<PathBuf>,
}

impl PaneSpec {
    /// A spec running `sh -c <command>` style shell text with the user's shell.
    #[must_use]
    pub fn shell_command(command: &str) -> Self {
        Self {
            argv: vec![user_shell(), "-c".into(), command.into()],
            cwd: None,
        }
    }

    /// The command to spawn, with an honest environment (G11): xterm-class
    /// `TERM`, `TERM_PROGRAM=monarch`, Monarch's own variables, and no
    /// variables from an enclosing tmux/screen/zellij.
    #[must_use]
    pub fn command(&self, pane: PaneId, env: &PaneEnv) -> CommandBuilder {
        let mut argv = self.argv.iter();
        let program = argv.next().cloned().unwrap_or_else(user_shell);
        let mut cmd = CommandBuilder::new(program);
        cmd.args(argv);
        if let Some(cwd) = self.cwd.clone().or_else(|| std::env::current_dir().ok()) {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "monarch");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        cmd.env("MONARCH_PANE", pane.to_string());
        cmd.env("MONARCH_SESSION", &env.session);
        cmd.env("MONARCH_SOCKET", &env.socket);
        for var in FOREIGN_VARS {
            cmd.env_remove(var);
        }
        cmd
    }
}

fn user_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    fn env() -> PaneEnv {
        PaneEnv {
            session: "main".into(),
            socket: "/tmp/m.sock".into(),
        }
    }

    #[test]
    fn environment_is_honest_and_identifies_the_pane() {
        let cmd = PaneSpec::default().command(PaneId(7), &env());
        assert_eq!(cmd.get_env("TERM"), Some(OsStr::new("xterm-256color")));
        assert_eq!(cmd.get_env("TERM_PROGRAM"), Some(OsStr::new("monarch")));
        assert_eq!(cmd.get_env("MONARCH_PANE"), Some(OsStr::new("p7")));
        assert_eq!(
            cmd.get_env("MONARCH_SOCKET"),
            Some(OsStr::new("/tmp/m.sock"))
        );
        assert_eq!(cmd.get_env("TMUX"), None);
    }

    #[test]
    fn argv_is_used_verbatim() {
        let spec = PaneSpec {
            argv: vec!["printf".into(), "%s".into(), "a b".into()],
            cwd: None,
        };
        let argv = spec.command(PaneId(1), &env()).get_argv().clone();
        assert_eq!(argv, ["printf", "%s", "a b"]);
    }

    #[test]
    fn shell_command_wraps_text_in_the_shell() {
        let spec = PaneSpec::shell_command("yes | head");
        assert_eq!(spec.argv[1..], ["-c".to_string(), "yes | head".into()]);
    }
}
