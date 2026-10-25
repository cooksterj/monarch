//! Where a session's socket lives.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::ServerError;

/// The longest socket path every supported OS accepts: macOS's `sun_path`
/// holds 104 bytes including the terminating NUL (Linux allows 108).
pub const MAX_SOCKET_PATH: usize = 103;

/// Resolves and prepares `<runtime dir>/<session>.sock`: under
/// `$XDG_RUNTIME_DIR/monarch/`, else `$TMPDIR/monarch-<uid>/`, else
/// `/tmp/monarch-<uid>/`, in a directory only the user can enter.
#[derive(Debug, Clone, Copy, Default)]
pub struct SocketPath;

impl SocketPath {
    /// The socket path for `session`, creating its private directory.
    ///
    /// # Errors
    /// The session name is not file-safe, the path would be too long for a
    /// Unix socket, or the directory cannot be made.
    pub fn for_session(session: &str) -> Result<PathBuf, ServerError> {
        Self::validate(session)?;
        let uid = rustix::process::getuid().as_raw();
        let dir = Self::dir_from(
            std::env::var_os("XDG_RUNTIME_DIR"),
            std::env::var_os("TMPDIR"),
            uid,
        );
        let path = Self::checked(dir.join(format!("{session}.sock")))?;
        Self::ensure_private_dir(&dir)?;
        Ok(path)
    }

    /// `path` if it fits in a Unix socket address.
    fn checked(path: PathBuf) -> Result<PathBuf, ServerError> {
        let len = path.as_os_str().len();
        if len > MAX_SOCKET_PATH {
            return Err(ServerError::SocketPathTooLong {
                path,
                len,
                max: MAX_SOCKET_PATH,
            });
        }
        Ok(path)
    }

    /// The directory every session's socket lives in, for this user.
    #[must_use]
    pub fn dir() -> PathBuf {
        let uid = rustix::process::getuid().as_raw();
        Self::dir_from(
            std::env::var_os("XDG_RUNTIME_DIR"),
            std::env::var_os("TMPDIR"),
            uid,
        )
    }

    /// Every session socket in [`Self::dir`] (some may be stale).
    #[must_use]
    pub fn all() -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(Self::dir()) else {
            return Vec::new();
        };
        let mut sockets: Vec<PathBuf> = entries
            .filter_map(|e| Some(e.ok()?.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "sock"))
            .collect();
        sockets.sort();
        sockets
    }

    /// Session names are file names: letters, digits, `-`, `_`, `.`.
    ///
    /// # Errors
    /// The name is empty, starts with a dot or has other characters.
    pub fn validate(session: &str) -> Result<(), ServerError> {
        if monarch_core::is_session_name(session) {
            Ok(())
        } else {
            Err(ServerError::InvalidSession(session.into()))
        }
    }

    fn dir_from(xdg: Option<OsString>, tmp: Option<OsString>, uid: u32) -> PathBuf {
        let present = |v: Option<OsString>| v.filter(|v| !v.is_empty()).map(PathBuf::from);
        if let Some(xdg) = present(xdg) {
            return xdg.join("monarch");
        }
        let base = present(tmp).unwrap_or_else(|| PathBuf::from("/tmp"));
        base.join(format!("monarch-{uid}"))
    }

    fn ensure_private_dir(dir: &Path) -> Result<(), ServerError> {
        fs::create_dir_all(dir)
            .and_then(|()| fs::set_permissions(dir, fs::Permissions::from_mode(0o700)))
            .map_err(|e| ServerError::io(format!("preparing {}", dir.display()), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn over_long_paths_are_refused_with_the_reason() {
        let short = PathBuf::from("/tmp/monarch-501/main.sock");
        assert_eq!(SocketPath::checked(short.clone()).ok(), Some(short));
        let long = PathBuf::from(format!("/tmp/{}.sock", "s".repeat(100)));
        let err = SocketPath::checked(long).expect_err("too long");
        assert!(err.to_string().contains("shorter session name"), "{err}");
    }

    #[test]
    fn prefers_xdg_runtime_dir_then_tmpdir_then_tmp() {
        let xdg = Some(OsString::from("/run/user/501"));
        let tmp = Some(OsString::from("/var/folders/x"));
        assert_eq!(
            SocketPath::dir_from(xdg, tmp.clone(), 501),
            Path::new("/run/user/501/monarch")
        );
        assert_eq!(
            SocketPath::dir_from(None, tmp, 501),
            Path::new("/var/folders/x/monarch-501")
        );
        assert_eq!(
            SocketPath::dir_from(Some(OsString::new()), None, 7),
            Path::new("/tmp/monarch-7")
        );
    }

    #[test]
    fn session_names_must_be_file_safe() {
        for good in ["main", "api-2", "a.b_c"] {
            assert!(SocketPath::validate(good).is_ok(), "{good}");
        }
        for bad in ["", ".hidden", "a/b", "x y", "../up"] {
            assert!(SocketPath::validate(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn private_dir_is_owner_only() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("monarch-test");
        SocketPath::ensure_private_dir(&dir).expect("created");
        let mode = fs::metadata(&dir).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }
}
