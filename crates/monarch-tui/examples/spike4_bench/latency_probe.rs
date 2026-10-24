//! Times a key from `ClientMsg::Input` until the pane's program reads it.
//!
//! The probe pane runs `cat` in raw mode into a FIFO, so each key comes back
//! out of band — never through rendering. The round trip (socket → server →
//! PTY `write(2)` → kernel → `cat` → FIFO) is an upper bound on the
//! key→`write(2)` latency the plan budgets.

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use monarch_core::PaneId;
use monarch_proto::ClientMsg;
use monarch_tui::ServerLink;

use super::stats::Stats;

/// The FIFO end of the probe pane.
pub struct LatencyProbe {
    fifo: File,
    pub pane: PaneId,
}

impl LatencyProbe {
    /// Creates the FIFO; [`Self::pane_command`] is what the pane must run.
    pub fn create(dir: &Path, pane: PaneId) -> std::io::Result<Self> {
        let path = Self::fifo_path(dir);
        let status = Command::new("mkfifo").arg(&path).status()?;
        if !status.success() {
            return Err(std::io::Error::other("mkfifo failed"));
        }
        // Read+write so opening does not block waiting for the pane.
        let fifo = OpenOptions::new().read(true).write(true).open(&path)?;
        Ok(Self { fifo, pane })
    }

    fn fifo_path(dir: &Path) -> PathBuf {
        dir.join("probe.fifo")
    }

    /// The probe pane's shell command.
    pub fn pane_command(dir: &Path) -> String {
        format!(
            "stty raw -echo; exec cat > '{}'",
            Self::fifo_path(dir).display()
        )
    }

    /// Sends `count` keys `gap` apart and records each round trip.
    pub fn measure(&mut self, link: &ServerLink, count: usize, gap: Duration) -> Stats {
        let mut stats = Stats::default();
        let mut byte = [0u8; 1];
        for _ in 0..count {
            let start = Instant::now();
            let sent = link.send(&ClientMsg::Input {
                pane: self.pane,
                bytes: b"x".to_vec(),
            });
            if sent.is_err() || self.fifo.read_exact(&mut byte).is_err() {
                break;
            }
            stats.push(start.elapsed());
            thread::sleep(gap);
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_fifo_the_pane_command_writes_to() {
        let dir = tempfile::tempdir().expect("tempdir");
        let _probe = LatencyProbe::create(dir.path(), PaneId(1)).expect("fifo");
        let cmd = LatencyProbe::pane_command(dir.path());
        assert!(cmd.starts_with("stty raw -echo; exec cat > '"), "{cmd}");
        assert!(cmd.ends_with("probe.fifo'"), "{cmd}");
        assert!(
            LatencyProbe::create(dir.path(), PaneId(1)).is_err(),
            "second mkfifo fails"
        );
    }
}
