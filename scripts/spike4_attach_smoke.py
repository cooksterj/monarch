#!/usr/bin/env python3
"""M1 spike 4: attach, detach, reattach and exit through the real binary.

Plays the host terminal for `monarch attach`: answers the startup probe,
types into the auto-started server's shell, detaches with Ctrl-b d,
reattaches to find the screen intact, then exits the shell and expects the
session to end.

Usage: python3 scripts/spike4_attach_smoke.py target/debug/monarch
"""

import fcntl
import os
import pty
import select
import struct
import sys
import tempfile
import termios
import time

HOST_REPLIES = b"\x1b]11;rgb:2828/2c2c/3434\x1b\\\x1b[?2026;2$y\x1b[?62;22c"


class Client:
    """One `monarch attach` run in a pseudo-terminal."""

    def __init__(self, binary, env):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execve(binary, [binary, "attach", "-s", "smoke"], env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
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

    def finish(self):
        deadline = time.time() + 5
        while time.time() < deadline:
            self.pump(0.05)
            done, status = os.waitpid(self.pid, os.WNOHANG)
            if done:
                self.pump(0.1)
                assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0, (status, self.seen[-300:])
                return
        raise AssertionError("client did not exit")


def main():
    binary = os.path.abspath(sys.argv[1])
    with tempfile.TemporaryDirectory(prefix="monarch-spike4-") as run:
        env = dict(os.environ, XDG_RUNTIME_DIR=run, SHELL="/bin/sh", PS1="$ ",
                   MONARCH_CONFIG=os.path.join(run, "none.toml"))
        env.pop("TMUX", None)

        first = Client(binary, env)
        # Typed at once: keys sent before the first layout must not be lost.
        os.write(first.fd, b"echo monarch-$((40+2))\r")
        first.wait_for(b"monarch-42", "echo output")
        os.write(first.fd, b"\x02d")
        first.wait_for(b"[detached from session smoke]", "detach message")
        first.finish()
        print("  attach: auto-started server, typed, detached")

        second = Client(binary, env)
        second.wait_for(b"monarch-42", "restored screen on reattach")
        os.write(second.fd, b"exit\r")
        second.wait_for(b"[session smoke ended]", "session end message")
        second.finish()
        assert not os.path.exists(os.path.join(run, "monarch", "smoke.sock")), "socket left behind"
        print("  reattach: screen restored; exiting the shell ends the session")
    print("spike4 attach smoke: ok")


if __name__ == "__main__":
    main()
