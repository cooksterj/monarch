//! [`GitRunner`] over the real `git` binary, so the user's git config and
//! version apply and nothing links libgit2.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::GitRunner;

/// Runs the `git` on `PATH`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CliGitRunner;

impl GitRunner for CliGitRunner {
    fn run(&self, dir: &Path, args: &[&str]) -> Option<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            // Reading never takes the index lock a user's git may want.
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_are_none() {
        let git = CliGitRunner;
        let missing = Path::new("/nonexistent/monarch-test-dir");
        assert_eq!(git.run(missing, &["rev-parse", "--show-toplevel"]), None);
        assert_eq!(git.run(Path::new("/"), &["no-such-subcommand"]), None);
    }
}
