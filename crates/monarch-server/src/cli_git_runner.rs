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
        self.try_run(dir, args).ok()
    }

    fn try_run(&self, dir: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run git: {e}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        } else {
            let why = String::from_utf8_lossy(&output.stderr);
            let why = why.trim().trim_start_matches("fatal: ");
            Err(if why.is_empty() {
                "git failed".to_owned()
            } else {
                why.to_owned()
            })
        }
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
        let why = git.try_run(missing, &["status"]).expect_err("fails");
        assert!(why.contains("monarch-test-dir"), "git's own message: {why}");
    }
}
