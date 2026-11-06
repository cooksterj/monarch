//! [`ProcessProbe`] over the real OS (`sysinfo`: `proc_pidinfo` on macOS,
//! `/proc` on Linux).

use std::path::PathBuf;
use std::sync::Mutex;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::ProcessProbe;

/// Asks the OS. Each query refreshes only the one process it reads. A
/// name is read into a system that has never seen the process: sysinfo
/// keeps the name a process had when first seen, so a probe that reused
/// it would miss an `exec` (`/bin/sh` on macOS execs bash; `exec claude`
/// from a shell) and keep naming the old program.
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

impl SystemProbe {
    fn lock(&self) -> std::sync::MutexGuard<'_, System> {
        self.system
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl ProcessProbe for SystemProbe {
    fn cwd(&self, pid: u32) -> Option<PathBuf> {
        let pid = Pid::from_u32(pid);
        let mut system = self.lock();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always),
        );
        system.process(pid)?.cwd().map(PathBuf::from)
    }

    fn name(&self, pid: u32) -> Option<String> {
        let pid = Pid::from_u32(pid);
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing(),
        );
        let name = system.process(pid)?.name().to_string_lossy();
        // A login shell is named `-zsh`.
        Some(name.trim_start_matches('-').to_owned()).filter(|n| !n.is_empty())
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
    fn a_missing_process_has_no_cwd_or_name() {
        assert_eq!(SystemProbe::new().cwd(u32::MAX - 1), None);
        assert_eq!(SystemProbe::new().name(u32::MAX - 1), None);
    }

    #[test]
    fn the_same_probe_sees_a_name_change_after_exec() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "exec sleep 5"])
            .spawn()
            .expect("sh");
        let probe = SystemProbe::new();
        let _first = probe.name(child.id());
        let name = (0..400)
            .map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(5));
                probe.name(child.id())
            })
            .find(|n| n.as_deref() == Some("sleep"))
            .flatten();
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(name.as_deref(), Some("sleep"), "one probe, name after exec");
    }

    #[test]
    fn reads_a_child_process_name() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .expect("sleep");
        // Until the child execs, it still carries the test thread's name.
        let name = (0..200)
            .map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(5));
                SystemProbe::new().name(child.id())
            })
            .find(|n| n.as_deref() == Some("sleep"))
            .flatten();
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(name.as_deref(), Some("sleep"));
    }
}
