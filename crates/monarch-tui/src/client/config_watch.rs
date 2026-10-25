//! Notices when the config file changes, so edits apply without a restart.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// How often the file's modification time is read.
pub const CONFIG_POLL: Duration = Duration::from_secs(1);

/// The config file and the modification time last seen (`None` while it
/// does not exist; creating or deleting it counts as a change).
#[derive(Debug, Clone)]
pub struct ConfigWatch {
    path: Option<PathBuf>,
    stamp: Option<SystemTime>,
    next: Instant,
}

impl ConfigWatch {
    /// Watches `path` from its current state; `None` watches nothing.
    #[must_use]
    pub fn new(path: Option<PathBuf>, now: Instant) -> Self {
        let mut watch = Self {
            path,
            stamp: None,
            next: now + CONFIG_POLL,
        };
        watch.stamp = watch.modified();
        watch
    }

    /// Whether the file changed since last asked; reads it at most every
    /// [`CONFIG_POLL`].
    pub fn changed(&mut self, now: Instant) -> bool {
        if self.path.is_none() || now < self.next {
            return false;
        }
        self.next = now + CONFIG_POLL;
        let stamp = self.modified();
        let changed = stamp != self.stamp;
        self.stamp = stamp;
        changed
    }

    fn modified(&self) -> Option<SystemTime> {
        let path = self.path.as_ref()?;
        std::fs::metadata(path).and_then(|m| m.modified()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creating_editing_and_deleting_are_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let t = Instant::now();
        let mut watch = ConfigWatch::new(Some(path.clone()), t);
        let later = |n: u32| t + CONFIG_POLL * n;
        assert!(!watch.changed(later(1)), "still missing");
        std::fs::write(&path, "[ui]\n").expect("write");
        assert!(!watch.changed(later(1)), "polled too soon");
        assert!(watch.changed(later(2)), "created");
        assert!(!watch.changed(later(3)), "untouched");
        let file = std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("open");
        file.set_modified(SystemTime::now() + Duration::from_secs(5))
            .expect("touch");
        assert!(watch.changed(later(4)), "edited");
        std::fs::remove_file(&path).expect("remove");
        assert!(watch.changed(later(5)), "deleted");
        assert!(
            !ConfigWatch::new(None, t).changed(later(9)),
            "nothing watched"
        );
    }
}
