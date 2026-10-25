#!/usr/bin/env python3
"""M1 spike 6: a session killed with `kill -9` comes back.

Plays the host terminal for `monarch attach` and checks that:

* the server keeps a state file with each pane's live working directory;
* after `kill -9` of the server, `monarch attach` restores the session: a
  shell comes back in the directory it had `cd`'d to, under a notice;
* a pane that ran a command comes back as a shell there, never re-running
  the command, and the notice names it;
* ending the session on purpose deletes the state file;
* a Claude Code pane starts with `--session-id <uuid>` and comes back with
  `--resume <uuid>`; a Codex pane comes back through `codex resume` (stand-in
  agent scripts record their arguments).

Usage: python3 scripts/spike6_restore_smoke.py target/debug/monarch
"""

import fcntl
import json
import os
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

# Unique per run, so cleanup can never touch a real session.
TAG = f"s6-{os.getpid()}-"
HOST_REPLIES = b"\x1b[?62;22c"
NOTICE = b"restored after the server stopped"


class Client:
    """One `monarch attach` run in a pseudo-terminal."""

    def __init__(self, binary, env, session, extra=()):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execve(binary, [binary, "attach", "-s", session, *extra], env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
        self.seen = b""
        self.wait_for(b"\x1b[c", "probe query")
        os.write(self.fd, HOST_REPLIES)

    def pump(self, seconds):
        ready, _, _ = select.select([self.fd], [], [], seconds)
        if ready:
            try:
                self.seen += os.read(self.fd, 65536)
            except OSError:
                pass

    def wait_for(self, needle, what, seconds=10):
        deadline = time.time() + seconds
        while needle not in self.seen:
            assert time.time() < deadline, f"timed out waiting for {what}: {self.seen[-300:]!r}"
            self.pump(0.05)

    def type(self, data):
        os.write(self.fd, data)

    def gone(self, seconds=5):
        deadline = time.time() + seconds
        while time.time() < deadline:
            self.pump(0.05)
            done, _ = os.waitpid(self.pid, os.WNOHANG)
            if done:
                return
        raise AssertionError("client did not exit")


def server_pid(session):
    out = subprocess.run(["pgrep", "-f", f"server --daemon --session {session}( |$)"],
                         capture_output=True, text=True).stdout.split()
    assert len(out) == 1, f"expected one server for {session}: {out}"
    return int(out[0])


def wait_for_state(path, predicate, what, seconds=10):
    deadline = time.time() + seconds
    while time.time() < deadline:
        try:
            state = json.load(open(path))
            if predicate(state):
                return state
        except (OSError, ValueError):
            pass
        time.sleep(0.1)
    raise AssertionError(f"state file never showed {what}")


def kill_server(session):
    pid = server_pid(session)
    os.kill(pid, signal.SIGKILL)
    deadline = time.time() + 5
    while subprocess.run(["kill", "-0", str(pid)], capture_output=True).returncode == 0:
        assert time.time() < deadline, "server survived kill -9"
        time.sleep(0.05)


def shell_comes_back_in_its_directory(binary, env, run):
    session = TAG + "sh"
    state = os.path.join(run, "monarch", session, "state.json")
    work = os.path.realpath(tempfile.mkdtemp(prefix="work-", dir=run))
    client = Client(binary, env, session)
    client.type(f"cd {work}\r".encode())
    wait_for_state(state, lambda s: s["panes"][0]["cwd"] == work, "the new directory")
    print("  state: the live working directory is saved")

    kill_server(session)
    client.gone()
    client = Client(binary, env, session)
    client.wait_for(NOTICE, "restore notice")
    client.type(b"pwd\r")
    client.wait_for(work.encode(), "restored directory")  # a fresh client: only pwd prints it
    print("  kill -9: the shell comes back in its directory, under a notice")

    client.type(b"exit\r")
    client.gone()
    deadline = time.time() + 5
    while os.path.exists(state):
        assert time.time() < deadline, "state file left after a clean exit"
        time.sleep(0.05)
    print("  exit: ending the session on purpose deletes its state")


def command_comes_back_as_a_shell(binary, env, run):
    session = TAG + "cmd"
    state = os.path.join(run, "monarch", session, "state.json")
    marker = os.path.join(run, "ran")
    # `--daemon` only leaves the terminal's session; it does not fork.
    subprocess.Popen([binary, "server", "--daemon", "--session", session,
                      "--pane", f"echo x >> {marker}; sleep 300"], env=env,
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL)
    wait_for_state(state, lambda s: len(s["panes"]) == 1, "the command pane")
    kill_server(session)
    client = Client(binary, env, session)
    client.wait_for(b"it was running: echo x", "notice naming the command")
    client.type(b"exit\r")
    client.gone()
    assert open(marker).read() == "x\n", "the command ran again"
    print("  command: comes back as a shell, not re-run; the notice names it")


FAKE_AGENT = """#!/bin/sh
echo "$*" >> "{log}"
echo "fake {name} $*"
exec sleep 300
"""


def lines(path, count, what, seconds=10):
    deadline = time.time() + seconds
    while True:
        try:
            got = open(path).read().splitlines()
            if len(got) >= count:
                return got
        except OSError:
            pass
        assert time.time() < deadline, f"{what} never ran"
        time.sleep(0.05)


def agents_resume_their_sessions(binary, env, run):
    bin_dir = os.path.join(run, "bin")
    os.mkdir(bin_dir)
    logs = {}
    for name in ("claude", "codex"):
        logs[name] = os.path.join(run, f"{name}.log")
        path = os.path.join(bin_dir, name)
        open(path, "w").write(FAKE_AGENT.format(log=logs[name], name=name))
        os.chmod(path, 0o755)
    env = dict(env, PATH=bin_dir + ":" + env["PATH"])
    session = TAG + "ag"
    state = os.path.join(run, "monarch", session, "state.json")
    client = Client(binary, env, session, ["--agent", "claude", "--agent", "codex"])
    saved = wait_for_state(state, lambda s: len(s["panes"]) == 2, "both agent panes")
    uuid = saved["panes"][0]["agent"]["session_id"]
    assert lines(logs["claude"], 1, "claude") == [f"--session-id {uuid}"], "claude launch argv"
    assert lines(logs["codex"], 1, "codex") == [""], "codex launches bare"
    assert saved["panes"][1]["agent"] == {"kind": "codex", "session_id": None}
    print("  agents: claude starts with --session-id <uuid>, saved with the pane")

    kill_server(session)
    client.gone()
    client = Client(binary, env, session)
    client.wait_for(b"resuming Claude Code session", "claude resume notice")  # the id wraps
    assert lines(logs["claude"], 2, "claude resume")[1] == f"--resume {uuid}"
    assert lines(logs["codex"], 2, "codex resume")[1] == "resume", "picker, never --last"
    print("  kill -9: claude resumes the same session; codex reopens its picker")
    os.kill(client.pid, signal.SIGKILL)
    os.waitpid(client.pid, 0)


def main():
    try:
        run_all(os.path.abspath(sys.argv[1]))
    finally:
        # A failed check leaves servers running; stop this run's servers.
        subprocess.run(["pkill", "-f", f"session {TAG}"], check=False)


def run_all(binary):
    with tempfile.TemporaryDirectory(prefix="monarch-spike6-") as run:
        # No sidebar: it would show the directory name, and the full path
        # printed by pwd must fit on one row.
        config = os.path.join(run, "config.toml")
        with open(config, "w") as f:
            f.write("[ui]\nsidebar = false\n")
        env = dict(os.environ, XDG_RUNTIME_DIR=run, XDG_STATE_HOME=run, SHELL="/bin/sh",
                   PS1="$ ", MONARCH_CONFIG=config)
        for var in ("MONARCH_PANE", "MONARCH_SESSION", "TMUX"):
            env.pop(var, None)
        shell_comes_back_in_its_directory(binary, env, run)
        command_comes_back_as_a_shell(binary, env, run)
        agents_resume_their_sessions(binary, env, run)
    print("spike 6 restore smoke: ok")


if __name__ == "__main__":
    main()
