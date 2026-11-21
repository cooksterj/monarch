# Architecture overview

How Monarch is put together. The emulator, input path and attach protocol are covered in more
depth in [terminal core](terminal-core.md); the reasons behind each choice are in
[decisions](../decisions.md).

## Shape

A client and server, like tmux. The **server** (one per session) owns the PTYs, the emulators,
agent state and persistence. The **client** is thin: it draws cell-grid deltas from the server and
sends input. Several clients can attach to one session.

```
monarch (bin) ──► monarch-cli   attach · new · ls · kill · hook · setup · doctor · agent · pane · tab
                     │
monarch-tui ◄── attach stream (postcard frames) ──► monarch-server (tokio)
                     JSON-lines API (CLI, hooks, agents) ─┘    ├─ monarch-term    Engine trait, alacritty backend, OscTap
                                                               ├─ monarch-detect  screen manifests, OSC signals
                                                               ├─ monarch-syntax  tree-sitter highlighting
                                                               └─ monarch-core    domain types, config, keymap, theme (no IO)
                              monarch-proto: wire types shared by all
```

| Crate | Owns |
|---|---|
| `monarch-core` | Ids, `AgentState`, `AgentKind`, `Action`, split tree and tabs, config, keymap and presets, themes, `Clock`. No IO. |
| `monarch-proto` | Attach messages (`ClientMsg`/`ServerMsg`, `Hello::WIRE`), JSON API types, `FrameCodec`. |
| `monarch-term` | The `Engine` trait, `AlacrittyEngine`, `OscTap`, selection and search. |
| `monarch-detect` | Herdr-compatible manifest loader and evaluator, the vendored manifests, `OscSignal`. |
| `monarch-server` | Sessions, `PaneRuntime`, `StateEngine`, frame coalescer, watchers, persister, worktrees, API and attach connections. |
| `monarch-syntax` | Syntax highlighting with tree-sitter: source text to lines of `SyntaxToken` runs. Grammars and queries for each `Language`. No IO, no colours. |
| `monarch-tui` | Raw input, prefix parser, `AttachState` (pure), views and widgets, `HostBackend`. |
| `monarch-cli` | The commands; `monarch hook` is small and fast (read stdin, post to the socket, exit). |

Packages publish as `monarch-mux-*`; `[lib] name` keeps imports as `monarch_core` and so on.
Git operations run the `git` command.

## Domain model

- A **session** (named, default `main`) holds **tabs**, each a split tree of **panes**.
- **Projects and worktrees** are derived from each pane's working directory (`git rev-parse`,
  `git status --porcelain=v2 --branch`), so the sidebar groups panes with no setup. `monarch new`
  and `prefix N` create a worktree under `[projects].worktree_root` (default
  `<repo>-worktrees/<branch>`) and open a tab in it.
- An **agent** is a pane running a known agent, launched by Monarch or recognised from its hooks,
  notifications or screen. It has a kind, a state, an activity line, its last message, a session id
  for resuming, timing, recent transitions and sub-agents.
- **`AgentState`**: `Working`, `Blocked` (waiting on the user), `Idle`, `Done` (a finished turn not
  yet seen), `Finished` (a Done the user has seen), `Error`, `Exited`, and `Unknown` for plain shells.

## Agent state

Four signals, most trusted first:

1. **Hooks.** `monarch setup` installs `monarch hook claude` and `monarch hook codex` for every event
   they read. A hook reads the event JSON on stdin and posts `agent.report` to the session socket:
   `UserPromptSubmit` → Working, tool events → activity, permission requests → Blocked, `Stop` →
   Done with the last message, `StopFailure` → Error, `SessionEnd` → shell. Events carrying
   `agent_id` describe a sub-agent's row instead of the parent. Claude Code hooks run in the pane and
   use its `MONARCH_PANE`; Codex reports carry no pane and the server places them.
2. **OSC notifications** caught by `OscTap` (OSC 9, 99, 777 and 9;4 progress), for agents without
   hooks.
3. **Screen manifests** (Herdr's format, 22 vendored) chosen by the foreground program, evaluated at
   most 4 times a second on the bottom of the buffer, never the scrolled viewport.
4. **Quiescence**, which only breaks ties.

A pane whose hooks have reported ignores the weaker signals. `monarch agent explain` and `prefix i`
show the state, which signal set it, every rule's outcome and the recent transitions.

## API

JSON lines over the session's Unix socket at `$XDG_RUNTIME_DIR/monarch/<session>.sock`, else under
`$TMPDIR` or `/tmp` in `monarch-<uid>/` (directory 0700, socket 0600). The first line is a JSON
hello choosing `json` or `attach`. Requests are `{id, method, params}`, answered by
`{id, result | error}`.

Methods: `agent.report`, `agent.explain`, `agent.wait`, `agent.read`, `pane.send`, `tab.rename`,
`worktree.create`, `session.info`, `session.kill`. The CLI wraps each (`monarch agent wait`,
`monarch pane send`, …), and the Monarch skill teaches agents to use them.

## Persistence

`$XDG_STATE_HOME/monarch/<session>/state.json` (else `~/.local/state/…`; directory 0700, file 0600,
written atomically). It holds tabs with their split trees, ratios, zoom, focus and names, and per
pane its argv, live working directory and agent (kind, session id, history). The persister compares
a capture every 2 s and writes only on change. A server started without `--pane` restores the saved
session; ending a session on purpose deletes the file. Programs, terminal state and screen contents
cannot survive the server, as with tmux.

## Terminal fidelity contract

The guarantees that make "never degrade the terminal" testable. CI checks them with the PTY
harnesses in `scripts/smoke_*.py`; the [roadmap](../roadmap.md) lists the manual checks still open.

| # | Guarantee | How |
|---|---|---|
| G1 | Keys reach the focused pane byte-exact | The client reads the tty raw on its own thread and forwards bytes straight to the socket, never through the render loop or crossterm's event parser. The prefix is one C0 byte (or a complete kitty sequence), so no Esc timer. Paste bodies are never scanned. |
| G2 | Kitty keyboard protocol works | Flags set absolutely with `CSI = bits;1 u` on focus or mode change, one push at start, pop at exit; never a push per pane. |
| G3 | Mouse behaves as in a bare terminal | The host reports SGR; the client re-encodes for the pane (SGR, UTF-8 or X10) in pane coordinates; the wheel scrolls history, or becomes arrows on the alternate screen under 1007. |
| G4 | Paste is intact | Bracketed paste mirrored; markers stripped unless the pane asked for them; never split mid-sequence. |
| G5 | Terminal queries are answered correctly | DA, DSR and kitty reports through the pane's writer in order; OSC 10/11/12 answered from the probed host colours; OSC 52 copies reach the host. |
| G6 | Focus events | Forwarded only if the pane asked; synthesised on pane switch. |
| G7 | Title and cursor | The focused pane's title goes to the host; cursor shape and visibility forwarded. |
| G8 | No tearing, no lag | Frames wrapped in DEC 2026 when the host supports it; 16 ms coalescing; the key path never touches rendering (p99 ≈ 0.25 ms measured). |
| G9 | Scrollback and reflow | 10k lines by default, reflow on resize, search and copy mode. |
| G10 | Selection survives output | Selection anchored to grid lines; Shift+drag and `prefix m` give the host's native selection. |
| G11 | The environment is honest | `TERM=xterm-256color`, `COLORTERM=truecolor`, `TERM_PROGRAM=monarch`, `MONARCH_PANE`, `MONARCH_SESSION`, `MONARCH_SOCKET`; `TMUX*`, `ZELLIJ*`, `STY` unset. |
| G12 | Process semantics | SIGWINCH on resize, SIGHUP on close, children reaped, widths consistent with the emulator. |
| G13 | Hidden panes stay correct but cheap | They keep parsing, so state and scrollback are right, but trigger no rendering. |

Known limits: no kitty graphics or sixel, no Windows.
