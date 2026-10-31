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
import re
import subprocess
import select
import struct
import sys
import tempfile
import termios
import time

# Unique per run, so cleanup can never touch a real session.
SESSION = f"smoke-{os.getpid()}"

# A cursor move (group 1), another escape sequence, or a run of text (group 2).
ESCAPE_OR_TEXT = re.compile(r"\x1b\[(\d+;\d+)H|\x1b(?:\[[0-9;?<=>$ ]*[A-Za-z@`~]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[()][0-9A-Za-z]|.)|([^\x1b]+)")

HOST_REPLIES = b"\x1b]11;rgb:2828/2c2c/3434\x1b\\\x1b[?2026;2$y\x1b[?62;22c"


class Client:
    """One `monarch attach` run in a pseudo-terminal."""

    def __init__(self, binary, env, session=SESSION, cols=80, cwd=None):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            if cwd:
                os.chdir(cwd)
            os.execve(binary, [binary, "attach", "-s", session], env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 24, cols, 0, 0))
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

    def idle(self, seconds):
        """Waits while reading output, as a real terminal would; a client
        whose output is not read blocks and falls behind."""
        deadline = time.time() + seconds
        while time.time() < deadline:
            self.pump(0.05)

    def wait_for(self, needle, what, seconds=10):
        deadline = time.time() + seconds
        while needle not in self.seen:
            assert time.time() < deadline, f"timed out waiting for {what}: {self.seen[-300:]!r}"
            self.pump(0.05)

    def wait_since(self, mark, needle, what, seconds=10):
        """Waits for `needle` in output after offset `mark`."""
        deadline = time.time() + seconds
        while needle not in self.seen[mark:]:
            assert time.time() < deadline, f"timed out waiting for {what}: {self.seen[-300:]!r}"
            self.pump(0.05)

    def wait_for_bar(self, text, what, seconds=10):
        """Waits for `text` on the bottom row (the tab bar). Only changed
        cells are redrawn, so the row is rebuilt from every write to it."""
        deadline = time.time() + seconds
        while text.decode() not in self.row(24):
            assert time.time() < deadline, f"timed out waiting for {what}: {self.row(24)!r}"
            self.pump(0.05)

    def row(self, number):
        """Row `number` (1-based) as drawn so far: cursor moves and text,
        other escape sequences skipped."""
        cells, row, col = {}, 0, 0
        for move, text in ESCAPE_OR_TEXT.findall(self.seen.decode(errors="replace")):
            if move:
                row, col = (int(n) for n in move.split(";"))
            elif row == number:
                for ch in text:
                    cells[col] = ch
                    col += 1
            else:
                col += len(text)
        return "".join(cells.get(c, " ") for c in range(1, max(cells, default=0) + 1))

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
    try:
        run_all(os.path.abspath(sys.argv[1]))
    finally:
        # A failed check leaves the server running; stop this run's server.
        subprocess.run(["pkill", "-f", f"session {SESSION}$"], check=False)


def run_all(binary):
    with tempfile.TemporaryDirectory(prefix="monarch-spike4-") as run:
        env = dict(os.environ, XDG_RUNTIME_DIR=run, XDG_STATE_HOME=run, SHELL="/bin/sh", PS1="$ ",
                   MONARCH_CONFIG=os.path.join(run, "none.toml"))
        env.pop("TMUX", None)

        first = Client(binary, env)
        # A fresh state directory is a first run: the welcome comes up, and
        # the key that closes it never reaches the pane.
        first.wait_for(b"press", "the welcome on the first attach")
        os.write(first.fd, b"q")
        first.idle(0.5)
        os.write(first.fd, b"echo monarch-$((40+2))\r")
        first.wait_for(b"monarch-42", "echo output")
        os.write(first.fd, binary.encode() + b"\r")
        first.wait_for(f"`{SESSION}`;".encode(), "nested attach refused")  # spaces are cursor moves
        first.wait_for(b"monarch \xe2\x94\x82 " + SESSION.encode(), "status line")
        first.wait_for(b"\x1b]2;monarch:" + SESSION.encode(), "window title")
        # The bar is rebuilt from all output so far: never reset `seen` first.
        first.wait_for_bar(b"sh ", "the tab named after the shell")
        os.write(first.fd, b"cat\r")
        first.wait_for_bar(b"cat", "the tab named after the program in front")
        os.write(first.fd, b"\x04")
        first.wait_for_bar(b"sh ", "the shell's name back")
        print("  names: the tab follows the foreground program (sh, cat, sh)")
        os.write(first.fd, b"\x02,")
        first.wait_for(b"rename tab 1", "the rename prompt")
        os.write(first.fd, b"api\r")
        first.wait_for_bar(b"1 api", "the tab's new name")
        os.write(first.fd, b"\x02,")
        first.idle(0.3)
        os.write(first.fd, b"\x15\r")  # Ctrl-u clears the name; Enter
        first.wait_for_bar(b"1 sh", "the automatic name back")
        os.write(first.fd, b"\x02.")
        first.wait_for(b"rename pane", "the rename-pane prompt")
        os.write(first.fd, b"server\r")
        first.wait_for_bar(b"1 server", "the focused pane's name on its tab")
        os.write(first.fd, b"\x02.")
        first.idle(0.3)
        os.write(first.fd, b"\x15\r")
        first.wait_for_bar(b"1 sh", "the pane's automatic name back")
        print("  rename: prefix , named the tab api, prefix . the pane server; blank names gave sh back")
        os.write(first.fd, b"\x02%")
        first.idle(0.5)
        os.write(first.fd, b"echo split-$((1+1))\r")
        first.wait_for(b"split-2", "output in the new split pane")
        os.write(first.fd, b"\x02\x1b[D")  # prefix, then the left arrow
        first.idle(0.3)
        os.write(first.fd, b"echo left-$((2+3))\r")
        first.wait_for(b"left-5", "output after focusing left")
        assert b"[Decho" not in first.seen, "the arrow leaked into the pane"
        print("  keys: prefix % split the window, prefix left focused the first pane")
        os.write(first.fd, b"\x02")
        first.wait_for(b"equalize", "which-key popup after the prefix")
        os.write(first.fd, b"c")
        first.idle(0.5)
        os.write(first.fd, b"echo tab-$((1+1))\r")
        first.wait_for(b"tab-2", "output in a new tab")
        first.seen = b""
        os.write(first.fd, b"\x021")
        first.wait_for(b"left-5", "first tab shown again")
        print("  tabs: the key popup listed bindings, prefix c opened a tab, prefix 1 went back")
        os.write(first.fd, b"\x02:")
        first.wait_for(b"command palette", "the command palette")
        os.write(first.fd, b"spl dow\r")
        first.idle(0.5)
        os.write(first.fd, b"echo palette-$((3+4))\r")
        first.wait_for(b"palette-7", "output in the pane the palette split off")
        first.seen = b""
        # Typed at once: what follows the picker key must reach the picker.
        os.write(first.fd, b"\x02g^2\r")
        first.wait_for(b"tab-2", "the go-to picker showed the second tab")
        os.write(first.fd, b"\x021")
        first.wait_for(b"left-5", "first tab shown again")
        print("  pickers: the palette ran split down, the go-to picker jumped to tab 2")
        os.write(first.fd, b"\x02")
        first.wait_for(b"PREFIX", "prefix indicator")
        os.write(first.fd, b"d")
        first.wait_for(f"[detached from session {SESSION}]".encode(), "detach message")
        first.finish()
        assert os.path.exists(os.path.join(run, "monarch", "welcomed")), "welcome remembered"
        assert b"qecho" not in first.seen, "the key closing the welcome leaked into the pane"
        print("  welcome: shown on the first attach only; the key that closed it was swallowed")
        print("  attach: auto-started server, nested attach refused, status line + title shown, typed, prefix shown, detached")

        second = Client(binary, env)
        second.wait_for(b"left-5", "restored screen on reattach")
        for _ in range(4):  # three split panes, then the second tab's shell
            os.write(second.fd, b"exit\r")
            second.idle(0.5)
        second.wait_for(f"[session {SESSION} ended]".encode(), "session end message")
        second.finish()
        assert not os.path.exists(os.path.join(run, "monarch", f"{SESSION}.sock")), "socket left behind"
        print("  reattach: screen restored; exiting every shell ends the session")
        two_clients_share_a_session(binary, env)
        sidebar_scrolls(binary, env)  # before the reload checks leave a bad config
        sidebar_groups_and_focuses(binary, env, run)
    print("spike4 attach smoke: ok")


def two_clients_share_a_session(binary, env):
    """Two terminals on one session see the same panes; the window fits
    the latest one, and fits the other again when it detaches."""
    session = f"{SESSION}-two"
    small = Client(binary, env, session=session, cols=80)
    # Typed at once: keys sent before the first layout must not be lost.
    os.write(small.fd, b"echo one-$((0+1))\r")
    small.wait_for(b"one-1", "output in the first client")
    big = Client(binary, env, session=session, cols=120)
    big.wait_for(b"one-1", "the same screen in the second client")
    mark_small, mark_big = len(small.seen), len(big.seen)
    os.write(small.fd, b"echo cols-$(tput cols)\r")
    big.wait_since(mark_big, b"cols-88", "typing in one client shows in the other")
    small.wait_since(mark_small, b"cols-", "the first client's own copy")
    os.write(big.fd, b"\x02d")
    big.wait_for(b"[detached", "second client detached")
    big.finish()
    small.idle(0.5)
    mark_small = len(small.seen)
    os.write(small.fd, b"echo cols-$(tput cols)\r")
    small.wait_since(mark_small, b"cols-80", "the window fits the client left")
    os.write(small.fd, b"exit\r")
    small.wait_for(b"ended]", "session end message")
    small.finish()
    print("  two clients: shared screen and input; the window followed the latest, then the one left")


def sidebar_groups_and_focuses(binary, env, run):
    """A 120-column host shows the sidebar: panes grouped by git checkout,
    a click focuses one, prefix t hides it and the panes get the room."""
    proj = os.path.join(run, "proj")
    os.mkdir(proj)
    subprocess.run(["git", "init", "-q", "-b", "trunk", proj], check=True)
    wide = Client(binary, env, session=f"{SESSION}-wide", cols=120, cwd=proj)
    wide.wait_for(b"proj", "the repository heading")
    wide.wait_for(b"trunk", "the branch")
    os.write(wide.fd, b"echo cols-$(tput cols)\r")
    wide.wait_for(b"cols-88", "panes beside a 32-column sidebar")
    os.write(wide.fd, b"\x02c")
    wide.idle(0.5)
    os.write(wide.fd, b"cd /\r")
    wide.wait_for(b"Scratch", "the pane outside a repository")
    wide.idle(0.3)
    mark = len(wide.seen)
    os.write(wide.fd, b"\x1b[<0;4;2M\x1b[<0;4;2m")  # "1 sh" under proj
    wide.idle(0.5)
    os.write(wide.fd, b"pwd\r")
    wide.wait_since(mark, b"/proj", "the clicked pane focused in tab 1")
    mark = len(wide.seen)
    os.write(wide.fd, b"\x02t")
    wide.idle(0.3)
    os.write(wide.fd, b"echo cols-$(tput cols)\r")
    wide.wait_since(mark, b"cols-120", "panes take the whole width")
    print("  sidebar: grouped by repo/branch, click focused a pane in another tab, prefix t hid it")
    config_reloads(wide, env["MONARCH_CONFIG"])
    for _ in range(2):
        os.write(wide.fd, b"exit\r")
        wide.idle(0.5)
    wide.wait_for(b"ended]", "session end message")
    wide.finish()


def sidebar_scrolls(binary, env):
    """More panes than sidebar rows: the sidebar follows focus to the
    newest pane, and the wheel over it scrolls it back to the top."""
    session = f"{SESSION}-sc"
    many = Client(binary, env, session=session, cols=120, cwd="/")
    many.wait_for(b"Scratch", "the sidebar")
    for _ in range(25):
        os.write(many.fd, b"\x02c")
        many.idle(0.15)
    many.idle(0.5)
    assert "Scratch" not in many.row(1), f"followed focus down: {many.row(1)!r}"
    for _ in range(10):
        os.write(many.fd, b"\x1b[<64;5;5M")  # wheel up over the sidebar
        many.idle(0.05)
    deadline = time.time() + 5
    while "Scratch" not in many.row(1):
        assert time.time() < deadline, f"wheel scrolled to the top: {many.row(1)!r}"
        many.pump(0.05)
    print("  sidebar scroll: followed focus past the bottom; the wheel scrolled it back")
    subprocess.run([binary, "kill", session], env=env, check=True, capture_output=True)
    deadline = time.time() + 5
    while not os.waitpid(many.pid, os.WNOHANG)[0]:
        assert time.time() < deadline, "client did not exit"
        many.pump(0.05)


def config_reloads(wide, path):
    """Edits to the config apply while attached; a bad edit is refused."""
    def edit(text, outcome=None):
        with open(path, "w") as f:
            f.write(text)
        if outcome:  # the row is rebuilt from all output: never reset it
            wide.wait_for_bar(outcome.encode(), f"notice: {outcome}")
        else:  # the same notice again draws nothing new; give it time
            wide.idle(2)

    def width(cols):
        mark = len(wide.seen)
        os.write(wide.fd, b"echo cols-$(tput cols)\r")
        wide.wait_since(mark, f"cols-{cols}".encode(), f"{cols} columns")

    os.write(wide.fd, b"\x02t")  # show the sidebar again
    width(88)
    edit("[ui]\nsidebar_width = 40\n", "config reloaded")
    width(80)
    edit('[keys]\nprefix = "ctrl+a"\n[ui]\nsidebar_width = 40\n')
    os.write(wide.fd, b"\x01t")
    width(120)
    edit('[keys]\nprefix = "alt+b"\n', "config not reloaded")
    os.write(wide.fd, b"\x01t")
    width(80)
    print("  reload: sidebar width and prefix changed live; a bad prefix kept the old keys")


if __name__ == "__main__":
    main()
