#!/usr/bin/env python3
"""M1 spike 3: input fidelity matrix and host mode mirroring, headless.

This script plays the host terminal. It runs the `single_pane` prototype in a
pseudo-terminal, answers its startup probe as a kitty-capable host would, and
runs a raw-mode recorder inside the pane. Then it types a matrix of keys and
checks that:

* the pane receives them byte-for-byte, except for the prefix and the
  host-only sequences the pane never asked for (paste markers, focus events);
* the pane's modes reach the host (DECCKM, keypad, kitty flags, mouse,
  DECSCUSR, title) and are undone on exit;
* the probed host background answers the pane's OSC 11 query.

Usage: python3 scripts/spike3_input_matrix.py target/debug/examples/single_pane
"""

import fcntl
import os
import pty
import re
import select
import struct
import sys
import tempfile
import termios
import time

HOST_REPLIES = (
    b"\x1b]10;rgb:dcdc/dfdf/e4e4\x1b\\\x1b]11;rgb:2828/2c2c/3434\x1b\\"
    b"\x1b]12;rgb:ffff/ffff/ffff\x1b\\\x1b[6;20;10t\x1b[?2026;2$y\x1b[?0u\x1b[?62;22c"
)

# Runs inside the pane: raw mode, set the requested modes, ask for the
# background colour, then append every byte received to a file.
RECORDER = r"""
import os, sys, tty
out, setup = sys.argv[1], bytes.fromhex(sys.argv[2])
tty.setraw(0)
os.write(1, setup + b"\x1b]11;?\x07READY\r\n")
with open(out, "ab", buffering=0) as f:
    while True:
        f.write(os.read(0, 65536))
"""

PASTE = b"x" * (1 << 20)


class Host:
    """The host terminal side of the prototype's pty."""

    def __init__(self, binary, setup):
        fd, self.dump = tempfile.mkstemp(prefix="monarch-spike3-")
        os.close(fd)
        args = [binary, sys.executable, "-c", RECORDER, self.dump, setup.hex()]
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execv(binary, args)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
        self.seen = b""
        self.wait_for(b"\x1b[c", "probe query")
        os.write(self.fd, HOST_REPLIES)
        self.wait_for(b"READY", "recorder start")

    def pump(self, seconds):
        deadline = time.time() + seconds
        while time.time() < deadline:
            ready, _, _ = select.select([self.fd], [], [], 0.02)
            if ready:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError:
                    return
                if not chunk:
                    return
                self.seen += chunk

    def wait_for(self, needle, what, seconds=5):
        deadline = time.time() + seconds
        while needle not in self.seen:
            assert time.time() < deadline, f"timed out waiting for {what}"
            self.pump(0.05)

    def send(self, data):
        view = memoryview(data)
        while view:
            _, writable, _ = select.select([], [self.fd], [], 0.02)
            if writable:
                view = view[os.write(self.fd, view[:4096]):]
            self.pump(0)

    def received(self, expected_len, seconds=10):
        """Bytes the pane got, minus its own OSC 11 reply."""
        deadline = time.time() + seconds
        while time.time() < deadline:
            self.pump(0.05)
            data = open(self.dump, "rb").read()
            body = re.sub(rb"\x1b\]11;[^\x07\x1b]*(\x07|\x1b\\)", b"", data, count=1)
            if len(body) >= expected_len:
                return data, body
        return data, body

    def quit(self, keys):
        os.write(self.fd, keys)
        deadline = time.time() + 5
        while time.time() < deadline:
            self.pump(0.1)
            done, status = os.waitpid(self.pid, os.WNOHANG)
            if done:
                os.unlink(self.dump)
                assert os.WIFEXITED(status) and os.WEXITSTATUS(status) == 0, status
                return
        raise AssertionError("prototype did not exit")


def check(host, sent, expected):
    data, body = host.received(len(expected))
    assert b"rgb:2828/2c2c/3434" in data, f"OSC 11 not answered from probe: {data[:80]!r}"
    if body != expected:
        at = next((i for i, (a, b) in enumerate(zip(body, expected)) if a != b), min(len(body), len(expected)))
        raise AssertionError(
            f"pane got {len(body)} bytes, expected {len(expected)}; first difference at {at}: "
            f"got {body[at:at+24]!r} expected {expected[at:at+24]!r}"
        )


def legacy_pane(binary):
    """A pane with no modes: keys pass, host-only sequences are stripped."""
    host = Host(binary, b"")
    keys = (
        b"\x1b" b"\x1bx" b"\x1b[A\x1bOA" b"\x1bOP\x1b[15~\x1b[24~" b"\x03\x7f\r"
        b"\x02\x02" b"\xc3\xa9\xf0\x9f\xa6\x8b" b"\x1b[I\x1b[O"
    )
    host.send(keys + b"\x1b[200~" + PASTE + b"\x1b[201~" + b"end")
    expected = keys.replace(b"\x02\x02", b"\x02").replace(b"\x1b[I\x1b[O", b"") + PASTE + b"end"
    check(host, keys, expected)
    host.quit(b"\x02q")
    assert b"\x1b[?2004h" in host.seen and b"\x1b[?1004h" in host.seen
    print("  legacy pane: keys byte-exact, 1 MiB paste intact, markers + focus stripped")


def modes_pane(binary):
    """A pane with app cursor, keypad, kitty, mouse, paste, focus, cursor, title."""
    setup = (
        b"\x1b[?1h\x1b=\x1b[?2004h\x1b[?1004h\x1b[>1u\x1b[?1000h\x1b[?1006h"
        b"\x1b[5 q\x1b]2;spike-title\x07"
    )
    host = Host(binary, setup)
    host.pump(0.3)
    for seq in [b"\x1b[?1h", b"\x1b=", b"\x1b[>0u", b"\x1b[=1;1u", b"\x1b[?1000h",
                b"\x1b[?1006h", b"\x1b[5 q", b"\x1b]2;spike-title"]:
        assert seq in host.seen, f"host never got {seq!r}"
    keys = b"\x1b[13;2u\x1bOA\x1b[I\x1b[<0;5;5M\x1b[<0;5;5m\x1b[98;5u\x1b[98;5u\x1b[27u"
    host.send(keys + b"\x1b[200~" + PASTE + b"\x1b[201~")
    expected = keys.replace(b"\x1b[98;5u\x1b[98;5u", b"\x1b[98;5u") + b"\x1b[200~" + PASTE + b"\x1b[201~"
    check(host, keys, expected)
    before_quit = len(host.seen)
    host.quit(b"\x1b[98;5uq")
    tail = host.seen[before_quit:]
    for seq in [b"\x1b[?1l", b"\x1b>", b"\x1b[?1000l", b"\x1b[<u", b"\x1b[?2004l", b"\x1b[0 q"]:
        assert seq in tail, f"host not restored: missing {seq!r}"
    print("  modes pane: modes mirrored + restored, kitty prefix works, markers kept")


def main():
    binary = sys.argv[1]
    legacy_pane(binary)
    modes_pane(binary)
    print("spike3 input matrix: ok")


if __name__ == "__main__":
    main()
