//! The `monarch server` under test, as a child process we can measure.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// A running server and where it listens.
pub struct ServerProc {
    child: Child,
    pub socket: PathBuf,
}

impl ServerProc {
    /// Starts `monarch server` with `panes` in a private runtime dir.
    pub fn start(monarch: &Path, run_dir: &Path, panes: &[String]) -> std::io::Result<Self> {
        let mut cmd = Command::new(monarch);
        cmd.args(["server", "--session", "bench"]);
        for pane in panes {
            cmd.args(["--pane", pane]);
        }
        let child = cmd
            .env("XDG_RUNTIME_DIR", run_dir)
            .env("XDG_STATE_HOME", run_dir)
            .env("MONARCH_CONFIG", run_dir.join("none.toml"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()?;
        let socket = run_dir.join("monarch").join("bench.sock");
        let deadline = Instant::now() + Duration::from_secs(5);
        while UnixStream::connect(&socket).is_err() {
            if Instant::now() > deadline {
                return Err(std::io::Error::other("server did not start"));
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(Self { child, socket })
    }

    /// User + system CPU seconds the server has used so far.
    pub fn cpu_seconds(&self) -> f64 {
        cpu_seconds(self.child.id()).unwrap_or(f64::NAN)
    }

    /// Resident memory in MiB.
    pub fn rss_mib(&self) -> f64 {
        ps_field(self.child.id(), "rss=")
            .and_then(|kb| kb.trim().parse::<f64>().ok())
            .map_or(f64::NAN, |kb| kb / 1024.0)
    }
}

impl Drop for ServerProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn ps_field(pid: u32, field: &str) -> Option<String> {
    let out = Command::new("ps")
        .args(["-o", field, "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8(out.stdout).ok()
}

/// macOS `ps -o time=` prints `[[H:]M]M:SS.ss` with centisecond resolution.
#[cfg(target_os = "macos")]
fn cpu_seconds(pid: u32) -> Option<f64> {
    parse_ps_time(&ps_field(pid, "time=")?)
}

/// `[[H:]M]M:SS.ss` → seconds.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_ps_time(text: &str) -> Option<f64> {
    text.trim().split(':').try_fold(0.0, |acc, part| {
        part.parse::<f64>().ok().map(|v| acc * 60.0 + v)
    })
}

/// Linux: utime + stime from `/proc/<pid>/stat`, in clock ticks (100 Hz).
#[cfg(not(target_os = "macos"))]
fn cpu_seconds(pid: u32) -> Option<f64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_name = &stat[stat.rfind(')')? + 2..];
    let fields: Vec<&str> = after_name.split_whitespace().collect();
    let ticks: f64 = fields.get(11)?.parse::<f64>().ok()? + fields.get(12)?.parse::<f64>().ok()?;
    Some(ticks / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_cpu_times_parse() {
        assert_eq!(parse_ps_time("0:01.50\n"), Some(1.5));
        assert_eq!(parse_ps_time("1:02:03.00"), Some(3723.0));
        assert_eq!(parse_ps_time("garbage"), None);
    }
}
