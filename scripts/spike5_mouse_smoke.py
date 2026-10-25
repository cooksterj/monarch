#!/usr/bin/env python3
"""M1 spike 5: mouse routing through the real binary, headless.

Plays the host terminal for `monarch attach` and checks that:

* Monarch captures the host mouse in SGR form (`[ui] mouse = "auto"`);
* the wheel scrolls a shell's history, and typing returns to live output;
* a pane that enabled SGR tracking gets SGR reports with pane coordinates;
* a pane that enabled X10 tracking gets X10 bytes (re-encoded from SGR);
* on the alternate screen without tracking, the wheel becomes arrow keys;
* drag and double-click select text and copy it to the host with OSC 52;
* a program's own OSC 52 copy (vim, nvim, tmux) reaches the host;
* `prefix m` hands the mouse to the host terminal and takes it back.

Usage: python3 scripts/spike5_mouse_smoke.py target/debug/monarch
"""

import base64
import fcntl
import os
import pty
import subprocess
import re
import select
import struct
import sys
import tempfile
import termios
import time

# Runs inside the pane: raw mode, set modes, then record every byte.
# Raw mode makes Ctrl-C a plain byte, so `Q` ends it and the tty is restored.
# Unique per run, so cleanup can never touch a real session.
TAG = f"s5-{os.getpid()}-"

RECORDER = r"""
import os, sys, termios, tty
out, setup = sys.argv[1], bytes.fromhex(sys.argv[2])
saved = termios.tcgetattr(0)
tty.setraw(0)
os.write(1, setup + b"READY\r\n")
try:
    with open(out, "ab", buffering=0) as f:
        while True:
            data = os.read(0, 4096)
            if data == b"Q":
                break
            f.write(data)
finally:
    os.write(1, b"\x1b[?1000l\x1b[?1006l\x1b[?1049l")
    termios.tcsetattr(0, termios.TCSADRAIN, saved)
"""


class Host:
    """The host terminal around one `monarch attach`."""

    def __init__(self, binary, run, session):
        # SSH_CONNECTION keeps copies on OSC 52 only: no pbcopy clobbering
        # the clipboard of whoever runs the tests.
        env = dict(os.environ, XDG_RUNTIME_DIR=run, XDG_STATE_HOME=run, SHELL="/bin/sh", PS1="$ ",
                   MONARCH_CONFIG=os.path.join(run, "none.toml"), SSH_CONNECTION="smoke")
        for var in ("MONARCH_PANE", "MONARCH_SESSION", "TMUX"):
            env.pop(var, None)
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execve(binary, [binary, "attach", "-s", TAG + session], env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        self.seen = b""
        self.wait_for(b"\x1b[c", "probe query")
        os.write(self.fd, b"\x1b[?62;22c")
        self.wait_for(b"\x1b[?1006h", "SGR mouse capture")

    def pump(self, seconds):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([self.fd], [], [], 0.05)
            if ready:
                try:
                    self.seen += os.read(self.fd, 65536)
                except OSError:
                    return

    def wait_for(self, needle, what, seconds=10):
        deadline = time.time() + seconds
        while needle not in self.seen:
            assert time.time() < deadline, f"timed out waiting for {what}: {self.seen[-200:]!r}"
            self.pump(0.05)

    def text_since(self, mark):
        return re.sub(rb"\x1b\[[0-9;?]*[a-zA-Z]", b" ", self.seen[mark:])

    def type(self, data):
        os.write(self.fd, data)

    def end(self):
        self.type(b"\x15exit\r")
        deadline = time.time() + 5
        while time.time() < deadline:
            self.pump(0.05)
            done, _ = os.waitpid(self.pid, os.WNOHANG)
            if done:
                return
        raise AssertionError("client did not exit")


def wheel_scrolls_history(host):
    host.type(b"seq 100\r")
    host.wait_for(b"100", "seq output")
    host.pump(0.3)
    mark = len(host.seen)
    host.type(b"\x1b[<64;5;5M" * 5)
    host.pump(0.5)
    view = host.text_since(mark)
    assert b"64" in view and b"86" in view, f"wheel did not scroll: {view[:200]!r}"
    mark = len(host.seen)
    host.type(b"\x15")  # Ctrl-U: a key that changes nothing visible
    host.pump(0.5)
    assert b"100" in host.text_since(mark), "typing did not return to live output"
    print("  wheel: scrolls shell history 15 lines; a key returns to live output")


def osc52(text):
    return b"\x1b]52;c;" + base64.b64encode(text.encode()) + b"\x1b\\"


def selection_copies(host):
    host.type(b"clear; echo alpha beta-gamma delta\r")
    host.wait_for(b"delta", "echo output")
    host.pump(0.5)
    # After `clear` the output is on the first row: "alpha beta-gamma delta".
    host.type(b"\x1b[<0;7;1M\x1b[<0;7;1m\x1b[<0;7;1M\x1b[<0;7;1m")
    host.wait_for(osc52("beta-gamma"), "double-click copy")
    host.wait_for(b"copied 10 charac", "copy notice")
    host.type(b"\x1b[<0;1;1M\x1b[<32;5;1M\x1b[<0;5;1m")
    host.wait_for(osc52("alpha"), "drag copy")
    print("  selection: double-click copies a word, drag copies a range (OSC 52), notice shown")


def drag_scrolls_past_the_edge(host):
    host.type(b"clear; seq 200\r")
    host.wait_for(b"200", "seq output")
    host.pump(0.5)
    mark = len(host.seen)
    # Press mid-screen, drag onto the top row and hold: the pane scrolls
    # back while the selection follows; the release copies all of it.
    host.type(b"\x1b[<0;1;12M\x1b[<32;1;1M")
    host.pump(2)
    host.type(b"\x1b[<0;1;1m")
    deadline = time.time() + 5
    while not (copies := re.findall(rb"\x1b\]52;c;([A-Za-z0-9+/=]+)", host.seen[mark:])):
        assert time.time() < deadline, "no OSC 52 after the drag"
        host.pump(0.05)
    text = base64.b64decode(copies[-1]).decode()
    lines = text.split("\n")
    # The screen started at line 178; slow machines scroll fewer lines.
    assert int(lines[0]) <= 172 and "185" in lines, f"drag did not reach scrolled-off lines: {text[:80]!r}"
    host.type(b"\x15")  # a key returns the pane to live output
    host.pump(0.3)
    print(f"  drag: holding past the top scrolled back; copied {len(lines)} lines from {lines[0]}")


def copy_mode_copies(host):
    host.type(b"clear; echo copy-me-now\r")
    host.wait_for(b"now", "echo output")
    host.pump(0.5)
    # Up to the output line, select to its end, yank.
    host.type(b"\x02[")
    host.wait_for(b"opy]", "copy-mode status")
    host.type(b"k0v$y")
    host.wait_for(osc52("copy-me-now"), "copy-mode yank")
    # A search moves the cursor to the match; the yank starts there.
    host.type(b"\x02[")
    host.wait_for(b"opy]", "copy-mode status again")
    host.type(b"/me-n\r")
    host.pump(0.5)
    host.type(b"v$y")
    host.wait_for(osc52("me-now"), "yank from a search hit")
    print("  copy mode: vi keys select and yank (OSC 52); / search moves the cursor")


def pane_copy_reaches_host(host):
    host.type(b"printf '\\033]52;c;%s\\007' $(printf from-vim | base64)\r")
    host.wait_for(osc52("from-vim"), "pane OSC 52 forwarded")
    print("  pane copy: a program's OSC 52 reaches the host clipboard")


def passthrough_toggles(host):
    mark = len(host.seen)
    host.type(b"\x02m")
    host.wait_for(b"(ctrl+b m", "passthrough notice")  # diffed draws skip unchanged cells
    assert b"\x1b[?1002l" in host.seen[mark:], "host mouse not released"
    mark = len(host.seen)
    host.type(b"\x02m")
    host.wait_for(b"\x1b[?1002h", "mouse recaptured")
    print("  prefix m: mouse handed to the host terminal and taken back")


def wait_since(host, mark, needle, what):
    deadline = time.time() + 10
    while needle not in host.seen[mark:]:
        assert time.time() < deadline, f"timed out waiting for {what}: {host.seen[-200:]!r}"
        host.pump(0.05)


def context_menu_runs_commands(host):
    host.type(b"clear\r")
    host.pump(0.5)
    # Right-click at host cell (9, 4): the menu opens one cell right of
    # and below it, so its first row ("split right") is on row 6.
    host.type(b"\x1b[<2;10;5M\x1b[<2;10;5m")
    host.wait_for(b"palette", "the context menu")
    host.type(b"\x1b")
    host.pump(0.3)
    mark = len(host.seen)
    host.type(b"echo menu-$((2+5))\r")
    wait_since(host, mark, b"menu-7", "typing after Esc closed the menu")
    host.type(b"\x1b[<2;10;5M\x1b[<2;10;5m")
    host.pump(0.3)
    host.type(b"\x1b[<0;14;7M\x1b[<0;14;7m")
    host.pump(0.5)
    mark = len(host.seen)
    host.type(b"echo cols-$(tput cols)\r")
    wait_since(host, mark, b"cols-", "output in the split pane")
    deadline = time.time() + 5
    while not re.search(rb"cols-(39|40)", host.seen[mark:]):
        assert time.time() < deadline, f"not split: {host.seen[mark:][-200:]!r}"
        host.pump(0.05)
    host.type(b"exit\r")
    host.pump(0.5)
    print("  context menu: right-click opened it, Esc closed it, a click on split right split the pane")


def recorder(host, run, name, setup):
    dump = os.path.join(run, name)
    open(dump, "wb").close()
    cmd = f"{sys.executable} -c '{RECORDER}' {dump} {setup.hex()}\r"
    host.type(cmd.encode())
    host.wait_for(b"READY", f"{name} recorder")
    host.pump(0.3)
    return dump


def received(dump, expected, seconds=5):
    deadline = time.time() + seconds
    while time.time() < deadline:
        data = open(dump, "rb").read()
        if data == expected:
            return
        time.sleep(0.05)
    raise AssertionError(f"pane got {data!r}, expected {expected!r}")


def tracking_pane(binary, run, name, setup, expected):
    host = Host(binary, run, name)
    dump = recorder(host, run, name, setup)
    host.type(b"\x1b[<0;5;3M\x1b[<0;5;3m")
    received(dump, expected)
    host.type(b"Q")
    host.pump(0.3)
    host.end()


def main():
    try:
        run_all(os.path.abspath(sys.argv[1]))
    finally:
        # A failed check leaves servers running; stop this run's servers.
        subprocess.run(["pkill", "-f", f"session {TAG}"], check=False)


def run_all(binary):
    with tempfile.TemporaryDirectory(prefix="monarch-spike5-") as run:
        host = Host(binary, run, "shell")
        wheel_scrolls_history(host)
        selection_copies(host)
        drag_scrolls_past_the_edge(host)
        copy_mode_copies(host)
        pane_copy_reaches_host(host)
        passthrough_toggles(host)
        context_menu_runs_commands(host)
        host.end()

        tracking_pane(binary, run, "sgr", b"\x1b[?1000h\x1b[?1006h",
                      b"\x1b[<0;5;3M\x1b[<0;5;3m")
        print("  SGR pane: click arrives as SGR with pane coordinates")
        tracking_pane(binary, run, "x10", b"\x1b[?1000h",
                      b"\x1b[M %#\x1b[M#%#")
        print("  X10 pane: click re-encoded from SGR to X10 bytes")
        host = Host(binary, run, "alt")
        dump = recorder(host, run, "alt", b"\x1b[?1049h")
        host.type(b"\x1b[<0;5;3M\x1b[<0;5;3m\x1b[<64;5;5M")
        received(dump, b"\x1b[A\x1b[A\x1b[A")
        host.type(b"Q")
        host.pump(0.3)
        host.end()
        print("  alt screen: wheel becomes 3 arrow keys; clicks are not typed")
    print("spike5 mouse smoke: ok")


if __name__ == "__main__":
    main()
