//! `[projects]`: where Monarch creates git worktrees.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Project and worktree configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectsConfig {
    /// Root for new worktrees. `None` places them next to the repository as
    /// `<repo>-worktrees/<name>`.
    pub worktree_root: Option<PathBuf>,
}

impl ProjectsConfig {
    /// Where a worktree named `name` for the repository at `repo` goes.
    #[must_use]
    pub fn worktree_path(&self, repo: &std::path::Path, name: &str) -> PathBuf {
        let repo_name = repo
            .file_name()
            .map_or_else(|| "repo".into(), |n| n.to_string_lossy().into_owned());
        match &self.worktree_root {
            Some(root) => root.join(&repo_name).join(name),
            None => repo
                .with_file_name(format!("{repo_name}-worktrees"))
                .join(name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_places_worktrees_beside_repo() {
        let p = ProjectsConfig::default().worktree_path(Path::new("/src/monarch"), "feat");
        assert_eq!(p, Path::new("/src/monarch-worktrees/feat"));
    }

    #[test]
    fn custom_root_groups_by_repo() {
        let cfg = ProjectsConfig {
            worktree_root: Some("/wt".into()),
        };
        let p = cfg.worktree_path(Path::new("/src/monarch"), "feat");
        assert_eq!(p, Path::new("/wt/monarch/feat"));
    }
}
