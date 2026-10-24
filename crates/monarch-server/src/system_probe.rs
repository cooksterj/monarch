//! [`ProcessProbe`] over the real OS (`sysinfo`: `proc_pidinfo` on macOS,
//! `/proc` on Linux).

use std::path::PathBuf;
use std::sync::Mutex;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::ProcessProbe;

/// Asks the OS. Each query refreshes only the one process it reads.
#[derive(Default)]
pub struct SystemProbe {
    system: Mutex<System>,
}

impl SystemProbe {
    /// A probe with nothing loaded yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProcessProbe for SystemProbe {
    fn cwd(&self, pid: u32) -> Option<PathBuf> {
        let pid = Pid::from_u32(pid);
        let mut system = self
            .system
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always),
        );
        system.process(pid)?.cwd().map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_this_process_cwd() {
        let probe = SystemProbe::new();
        let cwd = std::env::current_dir().expect("cwd");
        let seen = probe.cwd(std::process::id()).expect("readable");
        assert_eq!(seen.canonicalize().ok(), cwd.canonicalize().ok());
    }

    #[test]
    fn a_missing_process_has_no_cwd() {
        assert_eq!(SystemProbe::new().cwd(u32::MAX - 1), None);
    }
}
