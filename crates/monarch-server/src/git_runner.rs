//! Runs `git` commands, behind a trait so repository logic is testable
//! without repositories.

use std::path::Path;

/// What the server asks git.
pub trait GitRunner: Send + Sync {
    /// Runs `git <args>` in `dir`; its trimmed output if it succeeded.
    fn run(&self, dir: &Path, args: &[&str]) -> Option<String>;

    /// Like [`Self::run`], but a failure says why (git's own message when
    /// the runner has one), for commands whose failure the user sees.
    ///
    /// # Errors
    /// The command failed.
    fn try_run(&self, dir: &Path, args: &[&str]) -> Result<String, String> {
        self.run(dir, args)
            .ok_or_else(|| format!("git {} failed", args.first().unwrap_or(&"")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    impl GitRunner for Echo {
        fn run(&self, dir: &Path, args: &[&str]) -> Option<String> {
            (!args.is_empty()).then(|| format!("{} {}", dir.display(), args.join(" ")))
        }
    }

    #[test]
    fn runners_are_usable_as_trait_objects() {
        let git: Box<dyn GitRunner> = Box::new(Echo);
        assert_eq!(
            git.run(Path::new("/w"), &["status"]).as_deref(),
            Some("/w status")
        );
        assert_eq!(git.run(Path::new("/w"), &[]), None, "a failed command");
        assert_eq!(
            git.try_run(Path::new("/w"), &[]),
            Err("git  failed".to_owned())
        );
        assert_eq!(
            git.try_run(Path::new("/w"), &["log"]).as_deref(),
            Ok("/w log")
        );
    }
}
