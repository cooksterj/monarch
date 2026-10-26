//! Monarch's agent skill: the `SKILL.md` built into this binary, and
//! putting it in (or taking it out of) an agent's skills directory.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use super::{ChangeKind, ConfigFile, config_file::sibling};

/// The skill as this version of Monarch ships it.
pub(crate) const SKILL: &str = include_str!("../../skills/monarch/SKILL.md");

/// Monarch's skill in one agent's skills directory
/// (`<skills>/monarch/SKILL.md`), as it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkillFile {
    file: ConfigFile,
}

impl SkillFile {
    /// Reads Monarch's skill under `skills`; it need not exist.
    pub fn read(skills: &Path) -> anyhow::Result<Self> {
        let file = ConfigFile::read(&skills.join("monarch").join("SKILL.md"))?;
        Ok(Self { file })
    }

    /// Where the skill lives.
    pub fn path(&self) -> &Path {
        &self.file.path
    }

    /// What installing (or, with `uninstall`, removing) it would do.
    pub fn plan(&self, uninstall: bool) -> ChangeKind {
        match (self.file.text.as_deref(), uninstall) {
            (None, false) => ChangeKind::Added,
            (Some(text), false) if text == SKILL => ChangeKind::Unchanged,
            (Some(_), false) => ChangeKind::Updated,
            (Some(_), true) => ChangeKind::Removed,
            (None, true) => ChangeKind::Unchanged,
        }
    }

    /// Does what [`plan`](Self::plan) said. A copy that differs from this
    /// version's is backed up first; returns the backup's path.
    pub fn apply(&self, kind: ChangeKind, stamp: u64) -> anyhow::Result<Option<PathBuf>> {
        let path = self.path();
        let dir = path.parent().unwrap_or(path);
        match kind {
            ChangeKind::Added | ChangeKind::Updated => {
                fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
                self.file.replace(SKILL, stamp)
            }
            ChangeKind::Removed => self.remove(dir, stamp),
            ChangeKind::Skipped | ChangeKind::Unchanged => Ok(None),
        }
    }

    /// Deletes the skill, keeping a copy if the user changed it, and its
    /// directory if nothing else is in it.
    fn remove(&self, dir: &Path, stamp: u64) -> anyhow::Result<Option<PathBuf>> {
        let path = self.path();
        let backup = if self.file.text.as_deref() == Some(SKILL) {
            None
        } else {
            let backup = sibling(path, &format!("monarch-backup-{stamp}"));
            fs::copy(path, &backup).with_context(|| format!("backing up {}", path.display()))?;
            Some(backup)
        };
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
        let _ = fs::remove_dir(dir);
        Ok(backup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_at(skills: &Path) -> SkillFile {
        SkillFile::read(skills).expect("read")
    }

    #[test]
    fn installs_once_then_removes_cleanly() {
        let home = tempfile::tempdir().expect("dir");
        let skills = home.path().join("skills");
        let skill = skill_at(&skills);
        assert_eq!(skill.plan(true), ChangeKind::Unchanged, "nothing to remove");
        assert_eq!(skill.plan(false), ChangeKind::Added);
        assert_eq!(skill.apply(ChangeKind::Added, 1).expect("added"), None);
        let path = skills.join("monarch/SKILL.md");
        assert_eq!(fs::read_to_string(&path).expect("written"), SKILL);
        let skill = skill_at(&skills);
        assert_eq!(skill.plan(false), ChangeKind::Unchanged);
        assert_eq!(skill.apply(ChangeKind::Removed, 2).expect("removed"), None);
        assert!(!skills.join("monarch").exists(), "its directory goes too");
    }

    #[test]
    fn a_changed_copy_is_backed_up_before_it_is_replaced_or_removed() {
        let home = tempfile::tempdir().expect("dir");
        let dir = home.path().join("monarch");
        fs::create_dir_all(&dir).expect("dir");
        fs::write(dir.join("SKILL.md"), "my notes").expect("write");
        let skill = skill_at(home.path());
        assert_eq!(skill.plan(false), ChangeKind::Updated);
        let backup = skill.apply(ChangeKind::Updated, 3).expect("updated");
        assert_eq!(backup, Some(dir.join("SKILL.md.monarch-backup-3")));
        fs::write(dir.join("SKILL.md"), "edited again").expect("write");
        let skill = skill_at(home.path());
        let backup = skill.apply(ChangeKind::Removed, 4).expect("removed");
        let backup = backup.expect("kept");
        assert_eq!(fs::read_to_string(backup).expect("backup"), "edited again");
        assert!(!dir.join("SKILL.md").exists());
        assert!(dir.exists(), "the backups stay");
    }
}
