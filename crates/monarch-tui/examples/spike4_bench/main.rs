//! M1 spike 4: latency and load across a 21-pane session and two clients.
//!
//! Starts `monarch server` with a latency probe pane, a churn pane (`seq`:
//! every line differs, so it really exercises deltas), N × `yes` and N idle
//! agent panes, attaches two protocol clients and reports, per scenario,
//! key→pane latency (p50/p99/max), server CPU and memory, and bytes per
//! second each client receives.
//!
//! ```text
//! cargo build --release --bin monarch --example spike4_bench
//! target/release/examples/spike4_bench target/release/monarch \
//!     [--samples 1000] [--yes 10] [--idle 10] [--agent 'claude']
//! ```
//!
//! Exit status is non-zero when a p99 exceeds 2 ms or a stalled client
//! does not resync.

mod bench_client;
mod latency_probe;
mod server_proc;
mod stats;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use bench_client::BenchClient;
use latency_probe::LatencyProbe;
use monarch_core::PaneId;
use monarch_proto::{ClientMsg, ScreenSize};
use server_proc::ServerProc;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const BUDGET: Duration = Duration::from_millis(2);
const GAP: Duration = Duration::from_millis(5);
const SETTLE: Duration = Duration::from_secs(1);
/// Output whose every line differs; `yes` repeats one line, which cell
/// deltas make almost free, so it cannot load the render path.
const CHURN: &str = "exec seq -f %.0f 1 1000000000000";
const CHURN_PANE: PaneId = PaneId(2);
const IDLE_AGENT: &str =
    "printf '\\033[?1049h\\033[2J\\033[H  idle agent\\r\\n> '; exec sleep 86400";

struct Args {
    monarch: PathBuf,
    samples: usize,
    yes: usize,
    idle: usize,
    agent: String,
}

/// Everything a scenario measures.
struct Bench {
    server: ServerProc,
    probe: LatencyProbe,
    a: BenchClient,
    b: BenchClient,
    samples: usize,
}

struct Outcome {
    name: &'static str,
    p50: Duration,
    p99: Duration,
    max: Duration,
    cpu: f64,
    a_rate: f64,
    b_rate: f64,
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("spike4_bench: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool> {
    let args = parse_args()?;
    let dir = tempfile::tempdir()?;
    let mut bench = Bench::start(&args, dir.path())?;
    println!(
        "spike 4: 1 probe + 1 churn + {} yes + {} idle ({}) panes, 2 clients, {} keys/scenario",
        args.yes, args.idle, args.agent, args.samples
    );
    thread::sleep(SETTLE);
    let quiet = bench.scenario("probe focused, yes hidden", PaneId(1))?;
    let busy = bench.scenario("churn focused, 2 clients render", CHURN_PANE)?;
    let (stalled, resynced) = bench.stall_scenario()?;
    let rss = bench.server.rss_mib();
    print_table(&[&quiet, &busy, &stalled]);
    println!("server RSS {rss:.0} MiB; stalled client resynced: {resynced}");
    let ok = [&quiet, &busy, &stalled].iter().all(|o| o.p99 <= BUDGET) && resynced;
    println!("{}", if ok { "PASS" } else { "FAIL (p99 budget 2 ms)" });
    Ok(ok)
}

impl Bench {
    fn start(args: &Args, dir: &Path) -> Result<Self> {
        let probe = LatencyProbe::create(dir, PaneId(1))?;
        let mut panes = vec![LatencyProbe::pane_command(dir), CHURN.to_owned()];
        panes.extend(std::iter::repeat_n("yes".to_owned(), args.yes));
        panes.extend(std::iter::repeat_n(args.agent.clone(), args.idle));
        let server = ServerProc::start(&args.monarch, dir, &panes)?;
        let size = ScreenSize::new(200, 50);
        let a = BenchClient::connect(&server.socket, size)?;
        let b = BenchClient::connect(&server.socket, size)?;
        Ok(Self {
            server,
            probe,
            a,
            b,
            samples: args.samples,
        })
    }

    fn scenario(&mut self, name: &'static str, focus: PaneId) -> Result<Outcome> {
        self.a.link.send(&ClientMsg::Focus(focus))?;
        thread::sleep(SETTLE);
        self.measure(name)
    }

    fn measure(&mut self, name: &'static str) -> Result<Outcome> {
        let (cpu0, a0, b0, t0) = (
            self.server.cpu_seconds(),
            self.a.bytes(),
            self.b.bytes(),
            Instant::now(),
        );
        let stats = self.probe.measure(&self.a.link, self.samples, GAP);
        let secs = t0.elapsed().as_secs_f64();
        if stats.len() < self.samples {
            return Err(format!(
                "{name}: only {} of {} keys came back",
                stats.len(),
                self.samples
            )
            .into());
        }
        Ok(Outcome {
            name,
            p50: stats.percentile(50.0),
            p99: stats.percentile(99.0),
            max: stats.percentile(100.0),
            cpu: (self.server.cpu_seconds() - cpu0) / secs * 100.0,
            a_rate: (self.a.bytes() - a0) as f64 / secs,
            b_rate: (self.b.bytes() - b0) as f64 / secs,
        })
    }

    /// Client B stops reading while the churn pane renders; client A must not
    /// notice, and B must get fresh snapshots once it reads again.
    fn stall_scenario(&mut self) -> Result<(Outcome, bool)> {
        self.a.link.send(&ClientMsg::Focus(CHURN_PANE))?;
        self.b.set_paused(true);
        thread::sleep(SETTLE);
        let outcome = self.measure("client B stalled, A typing")?;
        let before = self.b.snapshots();
        self.b.set_paused(false);
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.b.snapshots() == before && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        Ok((outcome, self.b.snapshots() > before))
    }
}

fn print_table(rows: &[&Outcome]) {
    println!(
        "{:<32} {:>8} {:>8} {:>8} {:>8} {:>11} {:>11}",
        "scenario", "p50", "p99", "max", "srv CPU", "A recv/s", "B recv/s"
    );
    for o in rows {
        println!(
            "{:<32} {:>8} {:>8} {:>8} {:>7.0}% {:>11} {:>11}",
            o.name,
            micros(o.p50),
            micros(o.p99),
            micros(o.max),
            o.cpu,
            rate(o.a_rate),
            rate(o.b_rate)
        );
    }
}

fn micros(d: Duration) -> String {
    format!("{}µs", d.as_micros())
}

fn rate(bytes_per_sec: f64) -> String {
    match bytes_per_sec {
        b if b >= 1e6 => format!("{:.1} MB", b / 1e6),
        b if b >= 1e3 => format!("{:.1} kB", b / 1e3),
        b => format!("{b:.0} B"),
    }
}

fn parse_args() -> Result<Args> {
    let mut it = std::env::args().skip(1);
    let monarch = it
        .next()
        .ok_or("usage: spike4_bench <monarch> [--samples N] [--yes N] [--idle N] [--agent CMD]")?;
    let mut args = Args {
        monarch: monarch.into(),
        samples: 1000,
        yes: 10,
        idle: 10,
        agent: IDLE_AGENT.into(),
    };
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--samples" => args.samples = value.parse()?,
            "--yes" => args.yes = value.parse()?,
            "--idle" => args.idle = value.parse()?,
            "--agent" => args.agent = value,
            _ => return Err(format!("unknown flag {flag}").into()),
        }
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_human_readable() {
        assert_eq!(micros(Duration::from_micros(250)), "250µs");
        assert_eq!(rate(37_500.0), "37.5 kB");
        assert_eq!(rate(2_500_000.0), "2.5 MB");
        assert_eq!(rate(12.0), "12 B");
    }
}
