//! Reading an agent's config file and replacing it safely: a timestamped
//! backup first, then an atomic rename that keeps the file's permissions.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::Context;

/// A config file as it was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigFile {
    /// Where it lives.
    pub path: PathBuf,
    /// Its text, or `None` if it doesn't exist yet.
    pub text: Option<String>,
}

impl ConfigFile {
    /// Reads `path`; a missing file is not an error.
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Ok(Self {
            path: path.to_owned(),
            text,
        })
    }

    /// Replaces the file with `text`, first copying the old one to
    /// `<name>.monarch-backup-<stamp>`; returns the backup's path.
    pub fn replace(&self, text: &str, stamp: u64) -> anyhow::Result<Option<PathBuf>> {
        let path = &self.path;
        let backup = match &self.text {
            Some(_) => {
                let backup = sibling(path, &format!("monarch-backup-{stamp}"));
                fs::copy(path, &backup)
                    .with_context(|| format!("backing up {}", path.display()))?;
                Some(backup)
            }
            None => None,
        };
        let temp = sibling(path, "monarch-tmp");
        fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
        if let Ok(meta) = fs::metadata(path) {
            fs::set_permissions(&temp, meta.permissions())?;
        }
        fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(backup)
    }
}

/// `path` with `.suffix` added to its file name.
pub(super) fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{suffix}"));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn replace_backs_up_and_keeps_permissions() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("settings.json");
        fs::write(&path, "old").expect("write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("chmod");
        let file = ConfigFile::read(&path).expect("read");
        let backup = file.replace("new", 7).expect("replace").expect("a backup");
        assert_eq!(backup, dir.path().join("settings.json.monarch-backup-7"));
        assert_eq!(fs::read_to_string(&backup).expect("backup"), "old");
        assert_eq!(fs::read_to_string(&path).expect("file"), "new");
        let mode = fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!dir.path().join("settings.json.monarch-tmp").exists());
    }

    #[test]
    fn missing_files_are_created_without_a_backup() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("config.toml");
        let file = ConfigFile::read(&path).expect("read");
        assert_eq!(file.text, None);
        assert_eq!(file.replace("x = 1\n", 1).expect("replace"), None);
        assert_eq!(fs::read_to_string(&path).expect("file"), "x = 1\n");
        assert!(ConfigFile::read(dir.path()).is_err(), "a directory");
    }
}
