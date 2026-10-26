#!/usr/bin/env python3
"""M6 performance pass: a 20-pane session through the real attach client.

Starts `monarch server` with 20 panes and attaches `monarch attach` in a
PTY at 200x50, the way a user would. For each scenario it reports the
client's and server's CPU (from cumulative CPU time), the bytes the client
writes to the host terminal per second, both processes' memory, and, when
there is a probe pane, the latency of a key typed into the client until it
comes out of the pane.

Scenarios:
  idle     20 idle panes (`sleep`), nothing happening.
  working  20 fake agents reporting Working through `monarch hook claude`,
           so the sidebar's working dots animate.
  busy     a focused probe pane, 9 working agents and 10 agents printing a
           line every 20 ms (a fast agent's rate) in other tabs, with keys
           typed through the client.
  visible  as busy, but the first tab is split four ways: three panes
           printing on screen beside the probe pane being typed into.

User state, config and sockets live in a temporary directory; nothing
touches the user's own Monarch state.

  python3 scripts/perf_attach.py target/release/monarch [--seconds 10]
"""
import argparse
import fcntl
import os
import pty
import select
import shutil
import struct
import subprocess
import tempfile
import termios
import time

ROWS, COLS = 50, 200
PANES = 20
WORKING = (
    "printf '{\"hook_event_name\":\"UserPromptSubmit\",\"prompt\":\"refactor\"}' "
    "| {monarch} hook claude; exec sleep 86400"
)
CHATTY = (
    "printf '{\"hook_event_name\":\"UserPromptSubmit\",\"prompt\":\"build\"}' "
    "| {monarch} hook claude; "
    "i=0; while :; do i=$((i+1)); echo \"step $i: compiling crate $((i % 97))\"; sleep 0.02; done"
)
IDLE = "exec sleep 86400"


def cpu_seconds(pid):
    out = subprocess.run(["ps", "-o", "time=", "-p", str(pid)],
                         capture_output=True, text=True).stdout.strip()
    if not out:
        return 0.0
    total = 0.0
    for part in out.replace("-", ":").split(":"):
        total = total * 60 + float(part)
    return total


def rss_mib(pid):
    out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)],
                         capture_output=True, text=True).stdout.strip()
    return int(out or 0) / 1024


class Session:
    """A server with `panes` plus one attached client in a PTY."""

    def __init__(self, monarch, name, panes, home):
        self.env = dict(os.environ, TERM="xterm-256color",
                        XDG_STATE_HOME=os.path.join(home, "state"),
                        XDG_CONFIG_HOME=os.path.join(home, "config"),
                        XDG_RUNTIME_DIR=os.path.join(home, "run"))
        os.makedirs(os.path.join(home, "state", "monarch"), exist_ok=True)
        os.makedirs(os.path.join(home, "run"), mode=0o700, exist_ok=True)
        # Seen the welcome already: it would take the first key.
        open(os.path.join(home, "state", "monarch", "welcomed"), "w").close()
        args = [monarch, "server", "--session", name]
        for pane in panes:
            args += ["--pane", pane]
        self.server = subprocess.Popen(args, env=self.env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)
        self.client, self.fd = pty.fork()
        if self.client == 0:
            os.execve(monarch, [monarch, "attach", "-s", name], self.env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        self.received = 0

    def pump(self, seconds):
        """Reads (and counts) what the client draws for `seconds`."""
        end = time.time() + seconds
        while time.time() < end:
            if select.select([self.fd], [], [], 0.01)[0]:
                try:
                    self.received += len(os.read(self.fd, 1 << 16))
                except OSError:
                    break

    def close(self):
        os.write(self.fd, b"\x02d")
        self.pump(0.5)
        self.server.terminate()
        try:
            self.server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.server.kill()


def measure(session, seconds, probe=None):
    """CPU, host bytes/s and memory over `seconds`; key latency if `probe`."""
    client0, server0 = cpu_seconds(session.client), cpu_seconds(session.server.pid)
    bytes0, start = session.received, time.time()
    latencies = []
    if probe is None:
        session.pump(seconds)
    else:
        while time.time() - start < seconds:
            sent = time.perf_counter()
            os.write(session.fd, b"x")
            while True:
                session.pump(0)
                if select.select([probe], [], [], 0.002)[0] and os.read(probe, 64):
                    latencies.append(time.perf_counter() - sent)
                    break
                if time.perf_counter() - sent > 1:
                    raise SystemExit("perf_attach: a key never reached the probe pane")
            session.pump(0.01)
    took = time.time() - start
    return {
        "client_cpu": (cpu_seconds(session.client) - client0) / took * 100,
        "server_cpu": (cpu_seconds(session.server.pid) - server0) / took * 100,
        "host_bps": (session.received - bytes0) / took,
        "client_rss": rss_mib(session.client),
        "server_rss": rss_mib(session.server.pid),
        "latencies": sorted(latencies),
    }


def scenario(monarch, name, panes, seconds, home, probe_path=None, typed=()):
    session = Session(monarch, name, panes, home)
    probe = None
    try:
        session.pump(1.5)
        for keys in typed:
            os.write(session.fd, keys)
            session.pump(0.5)
        session.pump(1.5)  # settle: first frames, hooks, git probes
        if probe_path:
            probe = os.open(probe_path, os.O_RDONLY | os.O_NONBLOCK)
        return measure(session, seconds, probe)
    finally:
        if probe is not None:
            os.close(probe)
        session.close()


def pct(values, q):
    if not values:
        return "-"
    return "%.0fµs" % (values[min(len(values) - 1, int(len(values) * q))] * 1e6)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("monarch")
    ap.add_argument("--seconds", type=float, default=10.0)
    args = ap.parse_args()
    monarch = os.path.abspath(args.monarch)
    home = tempfile.mkdtemp(prefix="monarch-perf-")
    fifo = os.path.join(home, "probe")
    os.mkfifo(fifo)
    working = WORKING.replace("{monarch}", monarch)
    chatty = CHATTY.replace("{monarch}", monarch)
    probe = "stty raw -echo; exec cat > " + fifo
    # Each new split is focused; the probe goes in last so it keeps focus.
    loop = chatty.split("; ", 1)[1].encode()
    splits = [b"\x02%", loop + b"\r", b"\x02\"", loop + b"\r", b"\x02%",
              probe.encode() + b"\r"]
    runs = [
        ("idle", [IDLE] * PANES, None, ()),
        ("working", [working] * PANES, None, ()),
        ("busy", [probe] + [working] * 9 + [chatty] * 10, fifo, ()),
        ("visible", [chatty] + [working] * 9 + [chatty] * 7, fifo, splits),
    ]
    print("20 panes, client attached at %dx%d, %.0f s per scenario" % (COLS, ROWS, args.seconds))
    print("%-9s %10s %10s %12s %10s %10s %8s %8s %8s" % (
        "scenario", "client CPU", "server CPU", "to host/s", "client RSS",
        "server RSS", "key p50", "key p99", "key max"))
    try:
        for i, (name, panes, probe_path, typed) in enumerate(runs):
            r = scenario(monarch, "perf%d-%d" % (os.getpid(), i), panes, args.seconds,
                         home, probe_path, typed)
            lat = r["latencies"]
            print("%-9s %9.1f%% %9.1f%% %10.1f kB %7.0f MiB %7.0f MiB %8s %8s %8s" % (
                name, r["client_cpu"], r["server_cpu"], r["host_bps"] / 1000,
                r["client_rss"], r["server_rss"], pct(lat, 0.5), pct(lat, 0.99),
                pct(lat, 1.0)))
    finally:
        shutil.rmtree(home, ignore_errors=True)


if __name__ == "__main__":
    main()
