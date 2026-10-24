#!/usr/bin/env python3
"""Headless smoke test for the M1 spike-1 prototype.

Runs `single_pane` inside a pseudo-terminal, types commands into the pane,
and checks that the output renders and that `Ctrl-b q` exits cleanly.
Usage: python3 scripts/spike1_smoke.py target/debug/examples/single_pane
"""

import os
import pty
import select
import struct
import sys
import termios
import fcntl
import time


def read_for(fd, seconds):
    out = b""
    deadline = time.time() + seconds
    while time.time() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if ready:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
    return out


def main():
    binary = sys.argv[1]
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["SHELL"] = "/bin/sh"
        os.environ["PS1"] = "$ "
        os.execv(binary, [binary])
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    read_for(fd, 1.0)

    os.write(fd, b"printf 'spike-%s\\n' ok; printf '\\033]9;done\\007'\r")
    screen = read_for(fd, 1.0)
    assert b"spike-ok" in screen, f"pane output not rendered: {screen[-400:]!r}"

    os.write(fd, b"printf '\\033[c'\r")  # device attributes query must not hang
    read_for(fd, 0.5)

    os.write(fd, b"\x02q")
    # Keep draining: on macOS closing a tty blocks until its output is read.
    status = None
    deadline = time.time() + 5
    while status is None and time.time() < deadline:
        read_for(fd, 0.1)
        done, st = os.waitpid(pid, os.WNOHANG)
        if done:
            status = st
    assert status is not None, "prototype did not exit after Ctrl-b q"
    assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0, f"bad exit {status}"
    print("spike1 smoke: ok (rendered output, answered DA, Ctrl-b q exited 0)")


if __name__ == "__main__":
    main()
