//! One pane as written to the state file, and how it comes back.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::PaneSpec;

/// What a pane ran and where, enough to start it again after the server
/// died.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedPane {
    /// Program and arguments as started; empty for the user's shell.
    pub argv: Vec<String>,
    /// The program's working directory when last seen.
    pub cwd: Option<PathBuf>,
}

impl PersistedPane {
    /// Records `spec`, with `cwd` (the live directory) if known.
    #[must_use]
    pub fn new(spec: &PaneSpec, cwd: Option<PathBuf>) -> Self {
        Self {
            argv: spec.argv.clone(),
            cwd: cwd.or_else(|| spec.cwd.clone()),
        }
    }

    /// The pane to start in its place. A shell comes back as a shell in the
    /// same directory. Any other command also comes back as a shell there,
    /// never re-run: it may not be safe to run twice (a deploy, a delete).
    #[must_use]
    pub fn restore(&self) -> PaneSpec {
        let argv = if self.is_shell() {
            self.argv.clone()
        } else {
            Vec::new()
        };
        PaneSpec {
            argv,
            cwd: self.cwd.clone().filter(|dir| dir.is_dir()),
        }
    }

    /// The line shown above the restored pane's first output.
    #[must_use]
    pub fn banner(&self) -> String {
        let mut text = String::from("\x1b[2mmonarch: restored after the server stopped");
        if !self.is_shell() {
            text.push_str("; it was running: ");
            text.push_str(&self.command_text());
        }
        text.push_str("\x1b[0m\r\n");
        text
    }

    /// A shell spec has no arguments: the configured shell or the default.
    fn is_shell(&self) -> bool {
        self.argv.len() <= 1
    }

    /// The command as the user typed it (`sh -c` text unwrapped).
    fn command_text(&self) -> String {
        match self.argv.as_slice() {
            [_, flag, text] if flag == "-c" => text.clone(),
            argv => argv.join(" "),
        }
        .chars()
        .filter(|c| !c.is_control())
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_comes_back_in_its_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pane = PersistedPane::new(&PaneSpec::default(), Some(dir.path().into()));
        let spec = pane.restore();
        assert!(spec.argv.is_empty());
        assert_eq!(spec.cwd.as_deref(), Some(dir.path()));
        assert!(!pane.banner().contains("running"));
    }

    #[test]
    fn a_command_comes_back_as_a_shell_and_is_named_in_the_banner() {
        let pane = PersistedPane::new(&PaneSpec::shell_command("make deploy"), None);
        assert!(pane.restore().argv.is_empty(), "never re-run");
        assert!(pane.banner().contains("it was running: make deploy"));
    }

    #[test]
    fn a_vanished_directory_is_dropped() {
        let pane = PersistedPane {
            argv: Vec::new(),
            cwd: Some("/nonexistent/monarch/dir".into()),
        };
        assert_eq!(pane.restore().cwd, None);
    }

    #[test]
    fn control_characters_never_reach_the_banner() {
        let pane = PersistedPane {
            argv: vec!["sh".into(), "-c".into(), "echo \x1b]2;x\x07".into()],
            cwd: None,
        };
        assert!(!pane.banner().contains('\x07'));
    }
}
