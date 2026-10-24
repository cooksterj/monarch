//! Where a session's state lives on disk, written atomically.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::{PersistedSession, ServerError, SocketPath};

/// `$XDG_STATE_HOME/monarch/<session>/state.json`, else
/// `~/.local/state/monarch/<session>/state.json` (the same on macOS and
/// Linux, so it is easy to find over SSH).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFile {
    path: PathBuf,
}

impl StateFile {
    /// A state file at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The default state file for `session`.
    ///
    /// # Errors
    /// The session name is not a valid file name, or no home directory is
    /// known.
    pub fn for_session(session: &str) -> Result<Self, ServerError> {
        SocketPath::validate(session)?;
        let base = std::env::var_os("XDG_STATE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/state")))
            .ok_or_else(|| ServerError::io("finding the state directory", no_home()))?;
        Ok(Self::new(
            base.join("monarch").join(session).join("state.json"),
        ))
    }

    /// The file's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The saved session, if there is one.
    ///
    /// # Errors
    /// The file exists but cannot be read or parsed.
    pub fn load(&self) -> Result<Option<PersistedSession>, ServerError> {
        let text = match fs::read(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(ServerError::io("reading session state", e)),
        };
        serde_json::from_slice(&text)
            .map(Some)
            .map_err(|e| ServerError::io("parsing session state", e.into()))
    }

    /// Writes `session` to a temporary file and renames it into place, so
    /// a crash mid-write never leaves a torn file. Only the user can read
    /// it: commands may carry secrets.
    ///
    /// # Errors
    /// The directory or file cannot be written.
    pub fn save(&self, session: &PersistedSession) -> Result<(), ServerError> {
        let io = |what: &str| {
            let what = what.to_owned();
            move |e| ServerError::io(what, e)
        };
        if let Some(dir) = self.path.parent() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(io("creating the state directory"))?;
        }
        let json = serde_json::to_vec_pretty(session)
            .map_err(|e| ServerError::io("encoding session state", e.into()))?;
        let tmp = self.path.with_extension("json.tmp");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(io("writing session state"))?;
        file.write_all(&json)
            .and_then(|()| file.sync_all())
            .map_err(io("writing session state"))?;
        fs::rename(&tmp, &self.path).map_err(io("saving session state"))
    }

    /// Deletes the file (the session ended on purpose). Missing is fine.
    pub fn remove(&self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn no_home() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "neither XDG_STATE_HOME nor HOME is set",
    )
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::PersistedPane;

    #[test]
    fn saves_privately_loads_and_removes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = StateFile::new(dir.path().join("s/state.json"));
        assert_eq!(file.load().expect("load"), None, "nothing saved yet");
        let session = PersistedSession::new(vec![PersistedPane::default()], Some(0));
        file.save(&session).expect("save");
        assert_eq!(file.load().expect("load"), Some(session));
        let mode = fs::metadata(file.path())
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        file.remove();
        assert!(!file.path().exists());
    }

    #[test]
    fn a_corrupt_file_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = StateFile::new(dir.path().join("state.json"));
        fs::write(file.path(), b"{not json").expect("write");
        assert!(file.load().is_err());
    }

    #[test]
    fn session_names_are_checked() {
        assert!(StateFile::for_session("../etc").is_err());
        let file = StateFile::for_session("main").expect("path");
        assert!(file.path().ends_with("monarch/main/state.json"));
    }
}
