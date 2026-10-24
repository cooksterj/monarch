//! The machine's own clipboard, for clients not running over SSH.

use std::io::Write;
use std::process::{Command, Stdio};

/// Copies text with the platform's clipboard command.
pub trait LocalClipboard {
    /// The command that reads the text on stdin, if one is available.
    fn command(&self) -> Option<Command>;

    /// Copies `text`; an error when no command exists or it failed.
    ///
    /// # Errors
    /// No clipboard command, or it could not be run.
    fn copy(&self, text: &str) -> std::io::Result<()> {
        let mut cmd = self
            .command()
            .ok_or_else(|| std::io::Error::other("no clipboard command"))?;
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "clipboard command failed: {status}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Cat(std::path::PathBuf);

    impl LocalClipboard for Cat {
        fn command(&self) -> Option<Command> {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(format!("cat > '{}'", self.0.display()));
            Some(cmd)
        }
    }

    struct Missing;

    impl LocalClipboard for Missing {
        fn command(&self) -> Option<Command> {
            None
        }
    }

    #[test]
    fn copy_pipes_text_to_the_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("clip");
        Cat(file.clone()).copy("héllo").expect("copies");
        assert_eq!(std::fs::read_to_string(file).expect("read"), "héllo");
    }

    #[test]
    fn no_command_is_an_error() {
        assert!(Missing.copy("x").is_err());
    }
}
