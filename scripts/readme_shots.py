#!/usr/bin/env python3
"""Regenerates the README's captures from the real binary.

Plays the host terminal for a real `monarch attach`: a PTY, an emulator (pyte)
reading what the client draws, and Chrome turning each screen into an image
cell by cell. Nothing is mocked except the agents: small scripts that print a
screen and report through the real `monarch hook` path, in throwaway git
repositories with real worktrees and real diffs. A `kill -9` of the real server
makes the restore capture. State and sockets live in a temporary directory;
your own Monarch state is not touched.

  python3 -m venv /tmp/shots && /tmp/shots/bin/pip install pyte
  cargo build --release
  /tmp/shots/bin/python scripts/readme_shots.py target/release/monarch docs/assets

Needs Google Chrome, libwebp's img2webp and rustc. Uses JetBrains Mono when
installed. `just readme-shots` runs it with whichever python3 has pyte.
"""
import argparse
import concurrent.futures as futures
import dataclasses
import fcntl
import html
import json
import os
import pty
import re
import select
import shutil
import struct
import subprocess
import tempfile
import termios
import time

import pyte

COLS, ROWS = 108, 26
CELL_W, CELL_H, FONT_PX = 9, 20, 15
PAD = 10
CAPTION_H = 44
SIDEBAR = (0, 0, 31, 15)            # the sidebar's rows in the demo session
TERM_FG, TERM_BG = "#d5d8e4", "#14151c"
CHROME = (shutil.which("google-chrome") or shutil.which("chromium")
          or "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
FONT = "'JetBrainsMono Nerd Font Mono','JetBrains Mono',Menlo,'DejaVu Sans Mono',monospace"
NAMED = {"black": "#1c1d26", "red": "#ff5f5f", "green": "#7ed37e", "brown": "#e8a33d",
         "blue": "#5fafff", "magenta": "#c792ea", "cyan": "#6fb3d2", "white": "#d5d8e4"}
# What a real kitty-less terminal answers to the client's startup probe.
HOST_REPLIES = (b"\x1b]10;rgb:d5d5/d8d8/e4e4\x1b\\\x1b]11;rgb:1414/1515/1c1c\x1b\\"
                b"\x1b]12;rgb:e8e8/a3a3/3d3d\x1b\\\x1b[6;40;18t\x1b[?2026;2$y\x1b[?62;22c")
# Sequences pyte would otherwise print as text.
UNSUPPORTED = re.compile(rb"\x1b\[[0-9]* q|\x1b\[[<>=?][0-9;]*u|\x1b\[\?[0-9;]*\$p")
PARTIAL_CSI = re.compile(rb"\x1b(\[[0-9;:?<>=$ ]*)?$")

# Reports find a Codex pane by the name of its foreground program, so the
# scripted agents run under this stub, built as `claude` and `codex`. Restore
# runs it as Monarch would run the real agent: `claude --resume <id>`.
STUB = """use std::process::Command;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let me = args.remove(0);
    let mut command = Command::new("python3");
    if args.first().is_some_and(|a| a == "python3") {
        command.args(&args[1..]);
    } else {
        command.arg(std::env::var("DEMO_RESUMED").unwrap_or_default()).arg(me).args(&args);
    }
    let _ = command.status();
}
"""

AGENT = r"""
import json, os, select, subprocess, sys
name, kind, cwd, ctl, monarch = sys.argv[1:]
os.chdir(cwd)
os.write(1, f"\x1b]2;{name}\x07".encode())
subprocess.run(["stty", "raw", "-echo"])
fifo = os.open(ctl, os.O_RDWR | os.O_NONBLOCK)
pending = None

def act(cmd):
    global pending
    if "screen" in cmd:
        text = "\x1b[?25l\x1b[2J\x1b[H" + cmd["screen"].replace("\n", "\r\n")
        os.write(1, text.encode())
    if "hook" in cmd:
        event = dict(cmd["hook"], cwd=os.getcwd())
        subprocess.run([monarch, "hook", kind], input=json.dumps(event).encode(),
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    pending = cmd.get("on_key", pending if "on_key" not in cmd else None)

buf = b""
while True:
    ready = select.select([fifo, 0], [], [])[0]
    if fifo in ready:
        buf += os.read(fifo, 1 << 16)
        *lines, buf = buf.split(b"\n")
        for line in lines:
            act(json.loads(line))
    if 0 in ready:
        os.read(0, 64)
        if pending:
            cmd, pending = pending, None
            act(cmd)
"""

RESUMED = r"""
import json, os, subprocess, sys, time
kind, args = os.path.basename(sys.argv[1]), " ".join(sys.argv[2:])
resuming = "resume" in args
said = "Conversation resumed." if resuming else "Working on it."
print(f"\x1b[?25l\x1b[32m⏺\x1b[0m {said}\n\n  \x1b[38;2;128;132;150m$ {kind} {args}\x1b[0m", flush=True)
events = [{"hook_event_name": "SessionStart"}] if resuming else [
    {"hook_event_name": "UserPromptSubmit", "prompt": "fix the backoff"},
    {"hook_event_name": "Stop", "last_assistant_message": "Done."}]
for event in events:
    subprocess.run([os.environ["DEMO_MONARCH"], "hook", kind], input=json.dumps(dict(event, cwd=os.getcwd())).encode(),
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(86400)
"""


def ansi(text):
    """`{name}` colour tags to truecolor SGR."""
    tags = {"o": "38;2;217;119;87", "d": "38;2;128;132;150", "g": "38;2;126;211;126",
            "b": "38;2;95;175;255", "y": "38;2;232;163;61", "w": "1", "x": "0"}
    return re.sub(r"\{(\w)\}", lambda m: f"\x1b[{tags[m.group(1)]}m", text)


def agent_screen(body, width, prompt="❯"):
    """A transcript with the prompt box at the bottom, as an agent draws it."""
    box = "{d}" + "─" * width + "{x}"
    lines = ansi(body + "\n\n" + box + "\n{d}" + prompt + "{x}\n" + box).split("\n")
    pad = ROWS - 1 - len(lines)
    return "\n".join(lines[:-3] + [""] * max(pad, 0) + lines[-3:])


SPLIT_W, FULL_W = 35, 72
A11Y = agent_screen("""{g}⏺{x} I'll label the nav landmarks,
  then run the a11y suite.

{g}⏺{x} {w}Read{x}(src/nav.tsx)
  {d}⎿  Read 84 lines{x}

{g}⏺{x} {w}Update{x}(src/nav.tsx)
  {d}⎿  Added 6 lines, removed 2 lines{x}

{g}⏺{x} {w}Bash{x}(npm run test:a11y)
  {d}⎿  31 passed, 2 failed{x}

{g}⏺{x} Two failures: the footer links
  have no accessible name.

{o}✻{x} Thinking… {d}(41s){x}""", SPLIT_W)
TESTS = agent_screen("""{g}⏺{x} I'll run the suite until it's
  stable.

{g}⏺{x} {w}Bash{x}(npm test)
  {d}⎿  212 passed, 1 flaky{x}

{g}⏺{x} The flake is a timer the test
  never awaits. Using fake timers.

{g}⏺{x} {w}Update{x}(src/cart.test.tsx)
  {d}⎿  Added 4 lines, removed 1 line{x}

{g}⏺{x} {w}Bash{x}(npm test -- --repeat 20)
  {d}⎿  Running… (38s){x}

{o}✻{x} Thinking… {d}(2m 08s){x}""", SPLIT_W)
RETRY_INTRO = """{g}⏺{x} {w}Read{x}(src/retry.rs)
  {d}⎿  Read 42 lines{x}

{g}⏺{x} {w}Update{x}(src/retry.rs)
  {d}⎿  Added 7 lines, removed 3 lines{x}

{g}⏺{x} The backoff test needs a clean build first."""
RETRY_WORK = agent_screen(RETRY_INTRO + "\n\n{o}✻{x} Thinking… {d}(6s · esc to interrupt){x}", FULL_W)
RETRY_ASK = ansi(RETRY_INTRO + "\n\n{d}" + "─" * FULL_W + """{x}
 {w}Bash command{x}

   cargo clean && cargo test retry
   {d}Clean and re-run the retry tests{x}

 Do you want to proceed?
 {b}❯ 1. Yes{x}
   2. Yes, and don't ask again for cargo commands
   3. No, and tell Claude what to do differently {d}(esc){x}

 {d}Esc to cancel · Tab to amend{x}""")
RETRY_GO = agent_screen(RETRY_INTRO + """

{g}⏺{x} {w}Bash{x}(cargo clean && cargo test retry)
  {d}⎿  Running… (4s){x}

{o}✻{x} Thinking… {d}(12s · esc to interrupt){x}""", FULL_W)
HEADER = agent_screen("""{g}⏺{x} I'll stop the header overlapping the nav.

{g}⏺{x} {w}Update{x}(src/header.tsx)
  {d}⎿  Added 3 lines, removed 2 lines{x}

{g}⏺{x} {w}Write{x}(test/header.ts)
  {d}⎿  Wrote 7 lines{x}

{g}⏺{x} {w}Bash{x}(npm test -- header)
  {d}⎿  12 passed{x}

{g}⏺{x} Fixed the overlap on narrow screens. The header now wraps below
  640px, and I added a regression test.""", FULL_W)
PAY = agent_screen("""{d}•{x} I'll make the charge endpoint idempotent.

{d}•{x} Ran {w}rg -n "charge" src/{x}
  {d}└ 14 matches{x}

{d}•{x} Edited src/charge.rs {g}(+31{x} {y}-4){x}

{b}•{x} Working {d}(1m 12s • esc to interrupt){x}""", FULL_W, "›")
DOCS = agent_screen("""{g}⏺{x} Rewrote the setup section of the docs. Ready for the next request.""", FULL_W)

# name: (agent, worktree, screen, prompt)
SCENE = {
    "a11y": ("claude", "storefront-worktrees/feat-a11y", A11Y, "label the nav landmarks"),
    "retry": ("claude", "checkout-api-worktrees/feat-retry", RETRY_WORK, "fix the retry backoff"),
    "pay": ("codex", "checkout-api-worktrees/fix-idempotency", PAY, "make charges idempotent"),
    "header": ("claude", "storefront-worktrees/fix-header", HEADER, "fix the header overlap"),
    "docs": ("claude", "storefront", DOCS, "rewrite the setup docs"),
}
SPLIT = {"tests": ("claude", "storefront-worktrees/feat-a11y")}
TABS = ["a11y", "retry", "pay", "header", "docs"]
STOP_HEADER = {"hook_event_name": "Stop", "last_assistant_message":
               "Fixed the overlap on narrow screens. The header now wraps below 640px."}
COMMAND = {"command": "cargo clean && cargo test retry"}
ASK_RETRY = {"hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": COMMAND}
RUN_RETRY = {"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": COMMAND}

RETRY_RS = """use std::time::Duration;

/// How long to wait before attempt `n`.
pub fn backoff(n: u32) -> Duration {
    Duration::from_millis(100 * 2u64.pow(n))
}

pub fn should_retry(status: u16) -> bool {
    status == 503
}
"""
RETRY_RS_NEW = """use std::time::Duration;

const CAP: Duration = Duration::from_secs(30);

/// How long to wait before attempt `n`, capped so a long outage can't
/// push retries out indefinitely.
pub fn backoff(n: u32) -> Duration {
    let wait = 100u64.saturating_mul(2u64.saturating_pow(n));
    Duration::from_millis(wait).min(CAP)
}

pub fn should_retry(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}
"""
HEADER_TSX = """export function Header({ user }: Props) {
  return (
    <header className="row">
      <Logo />
      <Nav />
      <UserMenu user={user} />
    </header>
  );
}
"""
HEADER_TSX_NEW = """export function Header({ user }: Props) {
  const narrow = useNarrow();
  return (
    <header className={narrow ? "wrap" : ""}>
      <Logo />
      <Nav compact={narrow} />
      <UserMenu user={user} />
    </header>
  );
}
"""
HEADER_TEST = """import { render } from "@testing-library/react";

test("wraps when narrow", () => {
  window.resizeTo(480, 800);
  const { container } = render(<Header user={user} />);
  expect(container.firstChild).toHaveClass("wrap");
});
"""


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


class World:
    """A temp dir, with throwaway repositories and worktrees if asked."""

    def __init__(self, root, repos=True, branches=True):
        self.root = root
        self.work = os.path.join(root, "work")
        os.makedirs(self.work, exist_ok=True)
        self.git_env = dict(os.environ, GIT_AUTHOR_NAME="Demo", GIT_AUTHOR_EMAIL="demo@example.com",
                            GIT_COMMITTER_NAME="Demo", GIT_COMMITTER_EMAIL="demo@example.com")
        if not repos:
            return
        self.repo("checkout-api", "src/retry.rs", RETRY_RS, branches and {
            "feat/retry": {"src/retry.rs": RETRY_RS_NEW},
            "fix/idempotency": {"src/charge.rs": "pub fn charge() {}\n"},
        })
        self.repo("storefront", "src/header.tsx", HEADER_TSX, branches and {
            "feat/a11y": {"src/nav.tsx": "export const Nav = () => <nav aria-label=\"Main\" />;\n"},
            "fix/header": {"src/header.tsx": HEADER_TSX_NEW, "test/header.ts": HEADER_TEST},
        })
        if not branches:
            return
        # An upstream, so the sidebar's commits-to-push badge is real.
        wt = os.path.join(self.work, "checkout-api-worktrees/feat-retry")
        bare = os.path.join(root, "origin.git")
        self.git(root, "init", "--bare", "-q", bare)
        self.git(wt, "remote", "add", "origin", bare)
        self.git(wt, "push", "-q", "-u", "origin", "feat/retry")
        write(os.path.join(wt, "CHANGES.md"), "Retry notes\n")
        self.git(wt, "add", "CHANGES.md")
        self.git(wt, "commit", "-q", "-m", "notes")

    def git(self, cwd, *args):
        subprocess.run(["git", *args], cwd=cwd, env=self.git_env, check=True, capture_output=True)

    def repo(self, name, path, text, branches):
        repo = os.path.join(self.work, name)
        write(os.path.join(repo, path), text)
        self.git(repo, "init", "-q", "-b", "main")
        self.git(repo, "add", ".")
        self.git(repo, "commit", "-q", "-m", "initial")
        for branch, files in (branches or {}).items():
            wt = os.path.join(self.work, f"{name}-worktrees", branch.replace("/", "-"))
            self.git(repo, "worktree", "add", "-q", "-b", branch, wt)
            for rel, content in files.items():
                write(os.path.join(wt, rel), content)


class Host:
    """A server, its panes and one attached client in a PTY."""

    def __init__(self, monarch, world, first_run=False, agents=(), cwd=None, cols=COLS):
        self.monarch, self.world, self.cols = monarch, world, cols
        root = world.root
        self.env = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor",
                        XDG_STATE_HOME=f"{root}/state", XDG_CONFIG_HOME=f"{root}/config",
                        XDG_RUNTIME_DIR=f"{root}/run", DEMO_MONARCH=monarch,
                        DEMO_RESUMED=f"{root}/resumed.py",
                        PATH=f"{root}/bin:{os.environ['PATH']}")
        os.makedirs(f"{root}/state/monarch", exist_ok=True)
        os.makedirs(f"{root}/run", mode=0o700, exist_ok=True)
        os.makedirs(f"{root}/ctl", exist_ok=True)
        self.set_theme("monarch-dark")
        if not first_run:
            open(f"{root}/state/monarch/welcomed", "w").close()
        self.script = f"{root}/agent.py"
        write(self.script, AGENT)
        write(f"{root}/resumed.py", RESUMED)
        write(f"{root}/stub.rs", STUB)
        os.makedirs(f"{root}/bin", exist_ok=True)
        for kind in ("claude", "codex"):
            subprocess.run(["rustc", "-O", "-o", f"{root}/bin/{kind}", f"{root}/stub.rs"], check=True)
        args = [monarch, "server", "--session", "main"]
        if agents:
            for agent in agents:
                args += ["--agent", agent]
        else:
            panes = ["cd work; PS1='$ ' exec bash --norc"] if first_run else [self.agent_cmd(n) for n in SCENE]
            for pane in panes:
                args += ["--pane", pane]
        self.server = subprocess.Popen(args, env=self.env, cwd=cwd or root, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1.0)
        self.attach()

    def attach(self):
        self.screen = pyte.Screen(self.cols, ROWS)
        self.stream = pyte.ByteStream(self.screen)
        self.held = b""
        self.client, self.fd = pty.fork()
        if self.client == 0:
            os.execve(self.monarch, [self.monarch, "attach", "-s", "main"], self.env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, self.cols, 0, 0))

    def set_theme(self, theme, animations=True):
        write(f"{self.world.root}/config/monarch/config.toml",
              f'[ui]\nanimations = {str(animations).lower()}\n'
              f'[theme]\nauto_switch = false\nname = "{theme}"\n')

    def agent_cmd(self, name):
        kind, rel = SCENE[name][:2] if name in SCENE else SPLIT[name]
        ctl = f"{self.world.root}/ctl/{name}"
        if not os.path.exists(ctl):
            os.mkfifo(ctl)
        cwd = os.path.join(self.world.work, rel)
        return (f"cd {cwd} && exec {self.world.root}/bin/{kind} "
                f"python3 {self.script} {name} {kind} {cwd} {ctl} {self.monarch}")

    def tell(self, name, **cmd):
        deadline = time.time() + 10
        while True:
            try:
                fd = os.open(f"{self.world.root}/ctl/{name}", os.O_WRONLY | os.O_NONBLOCK)
                break
            except OSError:  # no reader yet: the agent is still starting
                if time.time() > deadline:
                    raise SystemExit(f"readme_shots: agent {name} never started")
                self.pump(0.1)
        os.write(fd, (json.dumps(cmd) + "\n").encode())
        os.close(fd)

    def pump(self, seconds, on_frame=None, tick=0.125):
        end = time.time() + seconds
        next_tick = time.time()
        while True:
            now = time.time()
            if now >= end:
                return
            if on_frame and now >= next_tick:
                on_frame(self.snapshot())
                next_tick = now + tick
            if select.select([self.fd], [], [], min(0.01, end - now))[0]:
                try:
                    data = os.read(self.fd, 1 << 16)
                except OSError:
                    return
                self.feed(data)

    def feed(self, data):
        if b"\x1b]10;?" in data:
            os.write(self.fd, HOST_REPLIES)
        data = self.held + data
        partial = PARTIAL_CSI.search(data)
        self.held = data[partial.start():] if partial else b""
        if partial:
            data = data[:partial.start()]
        self.stream.feed(UNSUPPORTED.sub(b"", data))

    def keys(self, data):
        os.write(self.fd, data)

    def snapshot(self):
        rows = []
        for y in range(ROWS):
            line = self.screen.buffer[y]
            rows.append([(line[x].data, line[x].fg, line[x].bg, line[x].bold, line[x].reverse)
                         for x in range(self.cols)])
        cursor = None if self.screen.cursor.hidden else (self.screen.cursor.x, self.screen.cursor.y)
        return rows, cursor

    def text(self):
        return "\n".join(self.screen.display)

    def close(self):
        monarch_out(self, "kill", "main")
        self.server.kill()
        self.server.wait()


def monarch_out(host, *args):
    return subprocess.run([host.monarch, *args], env=host.env, capture_output=True, text=True).stdout


def type_text(host, text):
    for ch in text.encode():
        host.keys(bytes([ch]))
        host.pump(0.03)


def colour(name, default):
    if name == "default":
        return default
    if name in NAMED:
        return NAMED[name]
    return f"#{name}" if re.fullmatch(r"[0-9a-fA-F]{6}", name) else default


def line_path(ch, x, y):
    """A box-drawing cell as an SVG path, so borders join with no gaps."""
    w, h = CELL_W, CELL_H
    cx, cy, r, b = x + w / 2, y + h / 2, x + w, y + h
    left, right, up, down = f"M{x} {cy}H{cx}", f"M{cx} {cy}H{r}", f"M{cx} {y}V{cy}", f"M{cx} {cy}V{b}"
    return {"─": left + right, "━": left + right, "│": up + down, "┃": up + down,
            "┌": right + down, "┐": left + down, "└": right + up, "┘": left + up,
            "├": up + down + right, "┤": up + down + left, "┬": left + right + down,
            "┴": left + right + up, "┼": left + right + up + down,
            "╭": f"M{cx} {b}V{cy + 4}Q{cx} {cy} {cx + 4} {cy}H{r}",
            "╮": f"M{cx} {b}V{cy + 4}Q{cx} {cy} {cx - 4} {cy}H{x}",
            "╰": f"M{cx} {y}V{cy - 4}Q{cx} {cy} {cx + 4} {cy}H{r}",
            "╯": f"M{cx} {y}V{cy - 4}Q{cx} {cy} {cx - 4} {cy}H{x}"}.get(ch)


def block_rect(ch, x, y):
    w, h = CELL_W, CELL_H
    return {"█": (x, y, w, h), "▀": (x, y, w, h / 2), "▄": (x, y + h / 2, w, h / 2),
            "▌": (x, y, w / 2, h), "▐": (x + w / 2, y, w / 2, h)}.get(ch)


def row_text(row):
    return "".join(cell[0] or " " for cell in row)


def box_crop(snap, title):
    """The cells of the pop-up with `title`, so a shot can show just it."""
    rows = snap[0]
    y, at = next((y, row_text(r).find(f" {title} ")) for y, r in enumerate(rows) if f" {title} " in row_text(r))
    left, right, bottom = at, at, y
    while rows[y][left][0] not in "┌╭":
        left -= 1
    while rows[y][right][0] not in "┐╮":
        right += 1
    while rows[bottom][left][0] not in "└╰":
        bottom += 1
    return (left, y, right + 1, bottom + 1)


def phone_crop(snap, caption):
    """What a phone-sized hero frame should show, given what's being said."""
    rows = snap[0]
    try:
        if caption in ("inbox", "answer"):
            return box_crop(snap, "inbox")
        if caption == "review":
            left, y, right, bottom = box_crop(snap, "review")
            divider = next(x for x in range(left + 1, right) if rows[y + 1][x][0] == "│")
            return (divider + 1, y + 1, right, min(y + 14, bottom))
    except StopIteration:      # the pop-up isn't open yet
        pass
    return SIDEBAR


@dataclasses.dataclass
class Shot:
    """How one screen is framed. A caption and a ring are annotations drawn
    around what Monarch drew, never inside it."""
    crop: tuple = None              # (first col, first row, end col, end row)
    canvas: tuple = None            # (cols, rows): scale the crop to fit this
    caption: tuple = None           # (keys, words)
    ring: str = None                # outlines the sidebar row naming this agent
    fg: str = TERM_FG
    bg: str = TERM_BG
    strip: str = "#0c0d12"
    caption_h: int = CAPTION_H


def cells(rows, cursor, shot):
    """The grid as absolutely placed cells, plus an SVG overlay for lines."""
    c0, r0, c1, r1 = shot.crop or (0, 0, len(rows[0]), len(rows))
    out, svg = [], []
    for ry in range(r0, r1):
        y = (ry - r0) * CELL_H
        for col in range(c0, c1):
            data, fg, bg, bold, rev = rows[ry][col]
            x = (col - c0) * CELL_W
            fgc, bgc = colour(fg, shot.fg), colour(bg, shot.bg)
            if rev or cursor == (col, ry):
                fgc, bgc = bgc, fgc
            if bgc != shot.bg:
                out.append(f'<i style="left:{x}px;top:{y}px;width:{CELL_W}px;height:{CELL_H}px;'
                           f'background:{bgc}"></i>')
            path, rect = line_path(data, x, y), block_rect(data, x, y)
            if path:
                weight = 2.2 if data in "━┃" else 1.1
                svg.append(f'<path d="{path}" stroke="{fgc}" stroke-width="{weight}" fill="none"/>')
            elif rect:
                svg.append('<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>'.format(*rect, fgc))
            elif data.strip():
                weight = "font-weight:700;" if bold else ""
                out.append(f'<b style="left:{x}px;top:{y}px;color:{fgc};{weight}">'
                           f"{html.escape(data)}</b>")
    return out, svg


def ring_mark(rows, shot):
    if not shot.ring:
        return ""
    c0, r0, _, r1 = shot.crop or (0, 0, len(rows[0]), len(rows))
    row = next(y for y in range(r0, r1) if f" {shot.ring} " in row_text(rows[y])[:31])
    return (f'<u style="left:{(1 - c0) * CELL_W}px;top:{(row - r0) * CELL_H - 1}px;'
            f'width:{29 * CELL_W}px;height:{CELL_H}px"></u>')


def page(snap, shot):
    """One screen as a self-contained HTML page, drawn cell by cell."""
    rows, cursor = snap
    out, svg = cells(rows, cursor, shot)
    c0, r0, c1, r1 = shot.crop or (0, 0, len(rows[0]), len(rows))
    width, height = (c1 - c0) * CELL_W, (r1 - r0) * CELL_H
    fit = ""
    grid_w, grid_h = width, height
    if shot.canvas:
        width, height = shot.canvas[0] * CELL_W, shot.canvas[1] * CELL_H
        scale = min(width / grid_w, height / grid_h, 1.0)
        fit = (f"transform:translate({(width - grid_w * scale) / 2}px,{(height - grid_h * scale) / 2}px) "
               f"scale({scale});transform-origin:0 0;")
    strip, total = "", height + 2 * PAD
    if shot.caption is not None:
        keys, words = shot.caption
        chip = f"<kbd>{html.escape(keys)}</kbd>" if keys else ""
        strip = f"<div class=c>{chip}<span>{html.escape(words)}</span></div>"
        total += shot.caption_h
    doc = f"""<!doctype html><meta charset=utf-8><style>
html,body{{margin:0;background:transparent}}
.t{{position:relative;box-sizing:border-box;width:{width + 2 * PAD}px;height:{total}px;
background:{shot.bg};border-radius:10px;border:1px solid #3a3d4e;overflow:hidden}}
.g{{position:absolute;left:{PAD - 1}px;top:{PAD - 1}px;width:{grid_w}px;height:{grid_h}px;{fit}}}
.g i,.g b,.g u{{position:absolute;display:block;font-style:normal;font-weight:400;text-decoration:none}}
.g b{{width:{CELL_W}px;height:{CELL_H}px;line-height:{CELL_H}px;text-align:center;overflow:visible;white-space:pre}}
.g u{{border:2px solid #e8a33d;border-radius:6px;box-sizing:content-box;margin:-1px 0 0 -4px;padding:0 2px}}
svg{{position:absolute;left:0;top:0}}
.c{{position:absolute;left:0;right:0;bottom:0;height:{shot.caption_h}px;box-sizing:border-box;padding:0 {PAD + 2}px;
display:flex;align-items:center;gap:12px;background:{shot.strip};border-top:1px solid #3a3d4e;color:{shot.fg};
font:17px/1.3 -apple-system,'Segoe UI',Helvetica,Arial,sans-serif}}
kbd{{font:600 15px {FONT};color:#1a1a1a;background:#e8a33d;padding:3px 9px;border-radius:5px;white-space:nowrap}}
</style><div class=t style="font:{FONT_PX}px {FONT}"><div class=g>{''.join(out)}
<svg width="{grid_w}" height="{grid_h}">{''.join(svg)}</svg>{ring_mark(rows, shot)}</div>{strip}</div>"""
    return doc, width + 2 * PAD, total


def chrome_shot(src, png, size, scale=1, transparent=True):
    args = [CHROME, "--headless=new", "--disable-gpu", "--no-first-run", "--hide-scrollbars",
            f"--force-device-scale-factor={scale}", f"--window-size={size[0]},{size[1]}",
            f"--screenshot={png}", f"file://{src}"]
    if transparent:
        args.insert(-2, "--default-background-color=00000000")
    with tempfile.TemporaryDirectory() as profile:
        subprocess.run(args + [f"--user-data-dir={profile}"], check=True, capture_output=True)


def render(snap, png, scale, shot=None):
    doc, width, height = page(snap, shot or Shot())
    src = png + ".html"
    write(src, doc)
    chrome_shot(src, png, (width, height), scale)
    os.remove(src)


def animate(frames, path, scale, work):
    """`frames` is [(snapshot, Shot, seconds)]; writes one animated WebP."""
    base = os.path.join(work, os.path.basename(path))
    names = [f"{base}.{i:04}.png" for i in range(len(frames))]

    def draw(job):
        (snap, shot, _), name = job
        render(snap, name, scale, shot)

    with futures.ThreadPoolExecutor(max_workers=6) as pool:
        list(pool.map(draw, zip(frames, names)))
    args = ["img2webp", "-loop", "0", "-lossless"]
    for (_, _, seconds), name in zip(frames, names):
        args += ["-d", str(round(seconds * 1000)), name]
    subprocess.run(args + ["-o", path], check=True, capture_output=True)


def start(host):
    """The opening scene: every agent on screen, each in its state."""
    for name, (_, _, screen, prompt) in SCENE.items():
        host.tell(name, screen=screen, hook={"hook_event_name": "UserPromptSubmit", "prompt": prompt,
                                             "session_id": f"demo-{name}"})
    host.tell("docs", hook={"hook_event_name": "SessionStart"})
    host.pump(0.5)
    host.keys(b"\x02%")                # split the first tab
    host.pump(0.8)
    host.keys(host.agent_cmd("tests").encode() + b"\r")
    host.pump(0.8)
    host.tell("tests", screen=TESTS, hook={"hook_event_name": "UserPromptSubmit", "session_id": "demo-tests",
                                           "prompt": "stabilise the suite"})
    host.pump(0.3)
    for tab, name in enumerate(TABS, 1):
        monarch_out(host, "tab", "rename", name, "--tab", str(tab), "-s", "main")
    host.keys(b"\x02.")                # name the split's panes
    host.pump(0.3)
    type_text(host, "tests")
    host.keys(b"\r")
    host.pump(0.5)
    host.keys(b"\x02h")
    host.pump(0.5)
    host.keys(b"\x02.")
    host.pump(0.3)
    type_text(host, "a11y")
    host.keys(b"\r")
    host.pump(2.5)


CAPTIONS = {
    "open": (None, "Six agents across two repositories. You are working in a11y."),
    "blocked": (None, "retry needs permission to run a command. It's in another tab, and the sidebar says so."),
    "inbox": ("ctrl+b a", "The inbox: everything waiting on you."),
    "answer": ("1", "Answer from the inbox. retry goes back to work, and you never left a11y."),
    "jump": ("enter", "header is done. Jump to it."),
    "review": ("ctrl+b v", "Read what it changed, file by file."),
    "back": ("ctrl+b 1", "Back to where you were."),
}


def record_hero(host, out, work):
    frames = []  # [snapshot, caption key, seconds on screen]
    now = ["open"]

    def frame(snap):
        if frames and frames[-1][0] == snap and frames[-1][1] == now[0]:
            frames[-1][2] += 0.125
        else:
            frames.append([snap, now[0], 0.125])

    def beat(caption, keys, seconds):
        now[0] = caption
        if keys:
            host.keys(keys)
        host.pump(seconds, frame)

    host.tell("retry", screen=RETRY_WORK, hook={"hook_event_name": "UserPromptSubmit", "prompt": "fix the retry backoff"})
    host.pump(0.8)
    beat("open", None, 2.6)
    host.tell("retry", screen=RETRY_ASK, hook=ASK_RETRY, on_key={"screen": RETRY_GO, "hook": RUN_RETRY})
    host.tell("header", screen=HEADER, hook=STOP_HEADER)
    beat("blocked", None, 3.4)
    now[0] = "inbox"
    host.keys(b"\x02")
    host.pump(0.2, frame)
    beat("inbox", b"a", 2.8)
    beat("answer", b"1", 3.4)
    beat("jump", b"\r", 2.4)
    now[0] = "review"
    host.keys(b"\x02")
    host.pump(0.15, frame)
    beat("review", b"v", 4.4)
    host.keys(b"q")
    host.pump(0.2, frame)
    now[0] = "back"
    host.keys(b"\x02")
    host.pump(0.15, frame)
    beat("back", b"1", 2.0)
    print(f"hero: {len(frames)} frames, {sum(f[2] for f in frames):.1f}s")

    def ring(caption):
        return "retry" if caption == "blocked" else None

    wide = [(s, Shot(caption=CAPTIONS[c], ring=ring(c)), t) for s, c, t in frames]
    animate(wide, os.path.join(out, "hero.webp"), 1.5, work)
    narrow = [(s, Shot(crop=phone_crop(s, c), canvas=SIDEBAR[2:], caption=CAPTIONS[c], ring=ring(c), caption_h=96), t)
              for s, c, t in frames]
    animate(narrow, os.path.join(out, "hero-phone.webp"), 2, work)


# name, terminal text, terminal background
THEMES = [("monarch-light", "#2b2b2b", "#fbfaf6"), ("catppuccin-mocha", "#cdd6f4", "#1e1e2e"),
          ("gruvbox-dark", "#ebdbb2", "#282828")]


def stills(host, out):
    def shot(name, crop=None, **look):
        snap = host.snapshot()
        render(snap, os.path.join(out, f"{name}.png"), 2, Shot(crop=crop, **look))

    def popup(name, title):
        shot(name, box_crop(host.snapshot(), title))

    host.set_theme("monarch-dark", animations=False)
    host.pump(8.0)                     # the reload notice clears
    shot("sidebar", SIDEBAR)
    host.tell("retry", screen=RETRY_ASK, hook=ASK_RETRY)
    host.tell("header", screen=HEADER, hook=STOP_HEADER)
    host.pump(1.2)
    social(host, out)
    host.keys(b"\x02a")
    host.pump(1.0)
    popup("inbox", "inbox")
    host.keys(b"\x1b")
    host.pump(0.5)
    host.keys(b"\x022")                # the retry tab: why it is blocked
    host.pump(0.6)
    host.keys(b"\x02i")
    host.pump(1.5)
    popup("agent-info", "agent info")
    host.keys(b"\x1b")
    host.pump(0.5)
    host.keys(b"\x024")                # the header tab: its real diff
    host.pump(0.6)
    host.keys(b"\x02v")
    host.pump(1.5)
    shot("review")
    host.keys(b"q")
    host.pump(0.5)
    host.keys(b"\x021")
    host.pump(0.4)
    host.keys(b"\x02N")
    host.pump(0.8)
    type_text(host, "feat/parser")
    host.pump(0.5)
    popup("worktree", "new worktree")
    host.keys(b"\x1b")
    host.pump(0.5)
    for theme, fg, bg in THEMES:
        host.set_theme(theme, animations=False)
        host.pump(2.5)
        shot(f"theme-{theme}", SIDEBAR, caption=(None, theme), fg=fg, bg=bg, strip=bg)
    host.set_theme("monarch-dark", animations=False)
    host.pump(8.0)


def record_restored(monarch, out):
    """Agents started the way the README says, a real `kill -9`, a real
    restore: Monarch runs `claude --resume <id>` and the stub prints it."""
    root = tempfile.mkdtemp(prefix="mon-", dir="/tmp")
    host = None
    try:
        world = World(root, branches=False)
        api, web = (os.path.join(world.work, name) for name in ("checkout-api", "storefront"))
        a11y = f"{web}-worktrees/feat-a11y"
        world.git(web, "worktree", "add", "-q", "-b", "feat/a11y", a11y)
        host = Host(monarch, world, agents=["claude", "claude"], cwd=a11y)
        host.pump(1.5)
        for branch, agent, repo in (("feat/retry", "claude", api), ("fix/idempotency", "codex", api),
                                    ("fix/header", "claude", web), ("docs/setup", "claude", web)):
            subprocess.run([monarch, "new", branch, "--agent", agent, "--detached", "-s", "main"],
                           env=host.env, cwd=repo, check=True, capture_output=True)
        for tab, name in enumerate(("a11y", "tests", "retry", "pay", "header", "docs"), 1):
            monarch_out(host, "tab", "rename", name, "--tab", str(tab), "-s", "main")
        host.pump(3.0)
        host.keys(b"\x02d")
        host.pump(1.0)
        print("after detaching:", monarch_out(host, "ls").strip())
        host.server.kill()
        host.server.wait()
        host.attach()                  # auto-starts a new server, which restores
        host.pump(4.0)
        host.keys(b"\x023")
        host.pump(1.5)
        print(host.text())
        snap = host.snapshot()
        render(snap, os.path.join(out, "restored.png"), 2, Shot(crop=(0, 0, COLS, 16)))
        render(snap, os.path.join(out, "restored-phone.png"), 2, Shot(crop=(32, 0, COLS, 7)))
    finally:
        if host:
            host.close()
        shutil.rmtree(root, ignore_errors=True)


def social(host, out):
    """The image GitHub shows when the repository's link is shared."""
    screen = page(host.snapshot(), Shot())[0]
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../docs/assets/logo.svg")) as f:
        logo = f.read()
    style, body = screen.split("</style>", 1)
    doc = f"""{style}
html,body{{width:1280px;height:640px;overflow:hidden;background:#0d0e13}}
.card{{position:absolute;left:64px;top:56px;width:520px;font-family:-apple-system,'Segoe UI',Helvetica,Arial,sans-serif;color:#f5f0e6}}
.card svg{{position:static;width:150px;height:150px;display:block;margin:0 0 18px -6px}}
h1{{font-size:68px;line-height:1;margin:0 0 20px;font-weight:700;letter-spacing:-.02em}}
p{{font-size:32px;line-height:1.28;margin:0;color:#d5d8e4;text-wrap:balance}}
.shot{{position:absolute;left:616px;top:96px;transform-origin:0 0;transform:scale(.8)}}
</style><div class=card>{logo}<h1>Monarch</h1><p>Every agent in its own pane, and you always see which one needs you.</p></div>
<div class=shot>{body}</div>"""
    src = os.path.join(out, "social.html")
    write(src, doc)
    chrome_shot(src, os.path.join(out, "social-preview.png"), (1280, 640), transparent=False)
    os.remove(src)


def record_welcome(monarch, out):
    """A true first run: one shell, and the welcome screen's real swarm."""
    root = tempfile.mkdtemp(prefix="mon-", dir="/tmp")
    host = None
    try:
        host = Host(monarch, World(root, repos=False), first_run=True, cols=118)
        host.pump(3.0)                 # the name finishes typing itself
        frames = []
        host.pump(4.0, frames.append)
        animate([(s, Shot(), 0.125) for s in frames], os.path.join(out, "welcome.webp"), 1.5, root)
    finally:
        if host:
            host.close()
        shutil.rmtree(root, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("monarch")
    ap.add_argument("out")
    ap.add_argument("--only", choices=["hero", "stills", "welcome", "restored"], help="capture just one set")
    ap.add_argument("--debug", action="store_true", help="print the opening screen and CLI output")
    args = ap.parse_args()
    monarch, out = os.path.abspath(args.monarch), os.path.abspath(args.out)
    os.makedirs(out, exist_ok=True)
    for tool, need in (("Chrome", CHROME), ("img2webp", shutil.which("img2webp")), ("rustc", shutil.which("rustc"))):
        if not need or not os.path.exists(need):
            raise SystemExit(f"readme_shots: {tool} not found")
    if args.only in (None, "welcome"):
        record_welcome(monarch, out)
    if args.only in (None, "restored"):
        record_restored(monarch, out)
    if args.only in ("welcome", "restored"):
        return
    root = tempfile.mkdtemp(prefix="mon-", dir="/tmp")
    host = None
    try:
        world = World(root)
        host = Host(monarch, world)
        host.pump(1.5)
        start(host)
        if args.debug:
            print(host.text())
            print(monarch_out(host, "ls"))
            print(monarch_out(host, "agent", "explain", "-s", "main"))
        if args.only in (None, "hero"):
            record_hero(host, out, root)
        if args.only in (None, "stills"):
            stills(host, out)
    finally:
        if host:
            host.close()
        shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    main()
