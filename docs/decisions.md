# Decisions

Why Monarch is built the way it is. One entry per decision: what was chosen, and why. Add an entry
when a PR makes a real choice between alternatives; don't record what the code or the changelog
already says. Dates are when the decision was made.

## Product

- **Win on attention, not features** (2026-10-23). Herdr is the incumbent (Rust, Apache-2.0, 22
  detection manifests, socket API, plugins). Monarch competes on seeing which agent needs you, and
  on never degrading the terminal: anything in a pane behaves as it would in a bare terminal.
- **Prefix-only keys, `Ctrl-b` by default, fully rebindable; presets `monarch`, `tmux`, `herdr`**
  (2026-10-23). Taking no keys from programs in panes is the choice most consistent with not
  degrading the terminal. The key popup recovers discoverability for the second key.
- **A TUI with a detachable server, like tmux** (2026-10-23). It runs in the user's own terminal and
  over SSH, and agents keep running when the client goes.
- **No notifications** (2026-10-25, user's decision: "a user can always just look at Monarch"). The
  sidebar and inbox are the attention surface; no toasts, host OSC forwarding or bells.
- **macOS and Linux only for v1** (2026-10-23). Unix sockets and PTYs; Windows later behind the same
  traits.
- **Named layouts parked** (2026-11-08, user's decision).
- **Next, set Monarch apart by taking agent work all the way to merged** (2026-11-19). The
  competitors now match its rows in the comparison table, and all of them stop at showing the diff.
  Monarch creates the worktrees and reads every agent's diff, so landing a change, warning about
  overlapping edits and best-of-N build on what it already has. Hook data powers the per-turn
  record, which tools that read the screen can't produce reliably. Several machines in one view
  and plugins stay out of scope: there, others are already ahead.
- **Ask from a selection comes first** (2026-11-21, user's decision). Getting code in front of an
  agent is the most frequent step in the loop and is copy, switch, paste today. `prefix e` was the
  free key in every preset. Enter sends at once only to an idle agent: typing into a blocked one
  could answer its approval prompt. With no agent there, it starts one rather than refusing. Starting
  agents with a prompt is also what best-of-N needs, so that work builds on this.

## Terminal

- **Rust** (2026-10-23). PTY and VT work is bound by throughput and latency, and it ships as one
  static binary. Python was rejected: `pyte` is slow and incomplete for full-screen agent TUIs and
  distribution is fragile.
- **`alacritty_terminal` 0.26 behind an `Engine` trait** (2026-10-23). libghostty-vt (Herdr's engine)
  needs Zig and network access at build time, which breaks `cargo install`. Ghostty can be an
  opt-in backend later.
- **Panes get `TERM=xterm-256color` and `TERM_PROGRAM=monarch`; Monarch never impersonates the host
  terminal** (2026-10-23). A program told it is in iTerm2 would send iTerm2's image and escape
  protocols, which Monarch can't honour. The cost: Claude Code sends no OSC 9 to Monarch, so hooks
  are the primary state signal.
- **An `OscTap` pre-parser rather than a second parser or a fork** (2026-10-23). vte's `Performer`
  is private and unknown OSCs are only logged, so a byte-level, `memchr`-gated scanner intercepts
  OSC 9/99/777/7/133/52; a second `vte::Parser` would cost 1.5–2× parse CPU on the hot path and is
  kept only as the test oracle.
- **Emoji presentation selectors make the glyph wide in the grid** (2026-11-12). Hosts, ratatui and
  Ink (Claude Code) give `⚠️` two cells; alacritty gave one, which put the program's later cursor
  moves on the wrong cells. `PresentationHandler` widens the pair as the emulator writes it.
- **The client re-homes the cursor after every non-ASCII glyph** (2026-11-08). Hosts disagree on
  some glyph widths; trusting the cursor only after ASCII confines a disagreement to that glyph's
  own cells instead of shifting the rest of the row.

## Server and protocol

- **Server-side emulation with cell-grid deltas to clients, not raw PTY bytes** (2026-10-23). The
  server must emulate anyway (agent state, restore, reading a pane); raw forwarding needs a
  re-serialiser and mode replay, and agent TUIs redraw far more bytes than their net change in
  cells.
- **postcard frames with a `u32` length** (2026-10-23). bincode, the first choice, is unmaintained
  (RUSTSEC-2025-0141). Any change to an attach message's shape bumps `Hello::WIRE`, so a client and
  server from different builds refuse each other with a message saying how to restart, instead of
  stalling on a frame they can't decode (2026-10-27).
- **No `skip_serializing_if` on types in the attach stream** (2026-10-25). postcard is not
  self-describing; an omitted field breaks decoding and the server drops the client.
- **Keys reach the pane without the session lock, and CPU or blocking work runs in
  `spawn_blocking`** (2026-10-26). In the 20-pane pass, rendering, watchers and the persister on
  tokio workers delayed keystroke tasks queued behind them (debug p99 1.5 ms → 0.26 ms after the
  fix).
- **The window follows the client that resized last** (2026-10-24), as tmux's `window-size latest`;
  when it detaches, the most recent remaining client's size wins.
- **Nesting is refused** (2026-10-23). `monarch` inside a pane would attach to itself and loop every
  key back into its own pane.

## Agent state

- **Hooks are authoritative; OSC notifications, then screen manifests, then quiescence follow**
  (2026-10-23). Hooks are precise and cheap; the others cover agents without hooks.
- **Vendor Herdr's detection manifests unmodified, with attribution** (2026-10-25). Twenty-two
  agents are detected on day one. Upstream risks found while writing fixtures are kept, not
  patched: Qwen's composer rule expects a line starting with `>`; Muse's and Hermes's working rules
  outrank their approval rules.
- **Codex hook reports carry no pane; the server places them** (2026-10-24). Codex runs hooks in one
  shared `codex app-server` whose environment belongs to whichever pane started it, so `monarch
  hook codex` offers the report to every session, which matches it by session id, then by a `codex`
  foreground process in the reported directory.
- **A seen Done becomes Finished, not Idle** (2026-10-30). Focusing a finished agent used to lose the
  fact that it had finished.
- **The sidebar stopwatch counts working time only** (2026-11-06). It pauses while the agent waits on
  the user and freezes when the turn ends; the inbox shows the wait instead.
- **Sub-agent rows stay until the user's next prompt** (2026-11-14). Prompts the agent sends itself
  (a sub-agent's or teammate's report) don't count, and a turn's end marks rows done rather than
  dropping them, because Claude's `background_tasks` list teammates under ids that are not their
  `agent_id`s.

- **A new agent gets the ask as its first argument, not typed in** (2026-11-22). Claude Code and
  Codex both take a prompt as their last argument and start their first turn with it. Typing it
  into the pane would mean waiting for the agent's prompt to appear and racing its start-up
  (trust dialogs, updates), and the same race would come back for every agent kind added later.
  The argument is only part of the first launch: restore rebuilds an agent's command from its
  config and resumes the conversation, so the prompt is never sent twice.

## Persistence

- **Restore re-runs only what is safe** (2026-10-23). A shell comes back as a shell in its directory;
  any other command comes back as a shell there, never re-run, under a notice naming what ran.
- **Claude Code panes start with `--session-id <uuid>` and resume with `--resume <uuid>`; Codex
  resumes by the session id its hooks report, else `codex resume` (its picker), never `--last`**
  (2026-10-23). `--last` could take another pane's session in the same directory. A Claude pane
  never prompted restarts with the same id, since it has no conversation to resume (2026-10-25).
- **Working directories come from `sysinfo`** (2026-10-23). `libproc`'s `pidcwd` is unimplemented on
  macOS and it needs bindgen.
- **The persister stops before the state file is deleted** (2026-10-27). Aborting it could not stop
  a save already under way, which wrote back the file of a session the user had ended.

## Worktrees and review

- **Review diffs are highlighted per hunk side, on the server** (2026-11-22). Each hunk's context
  and added lines are highlighted together as the new code, and its context and removed lines as
  the old, so tree-sitter sees whole statements. Highlighting lines one by one was the other
  choice, but it loses strings and comments that span lines. The server sends the spans with the
  diff, as the viewer's are sent (the client stays free of grammars). Added and removed rows keep
  their meaning through a background tint blended from the theme's `popup` and `added`/`removed`
  tokens, not new colour tokens.
- **The file viewer highlights with tree-sitter, in its own crate** (2026-11-21). `syntect` was the
  other choice, but it only parses through `bincode` 1.x, which is unmaintained, and the user's
  rule is maintained dependencies only. tree-sitter's highlighter and the official grammars are
  maintained and MIT-licensed. The grammars are C compiled at build time (no Zig). Highlighting
  lives in `monarch-syntax`, with no IO and no colours, so its build dependencies stay in one place
  and the review can reuse it. The server reads and highlights off the async runtime and sends
  `SyntaxToken` runs, and the client colours them from the theme.

- **Git runs as the `git` command, not libgit2** (2026-10-23). Fewer build dependencies, and it
  honours the user's git configuration.
- **No fetch before creating a worktree** (2026-11-19). It would put a network round trip in every
  new worktree; a branch only on a remote is checked out tracking it, and the guide says to fetch
  first for one pushed since.
- **Removing a worktree never forces and keeps the branch** (2026-10-25). A dirty worktree is
  refused with the reason.
- **The editor runs as `/bin/sh -c '<editor> "$@"'`** (2026-10-25), as git runs `$EDITOR`, so
  `code --wait` works and the path is never shell text.

## Configuration

- **A bad setting is a warning, never a crash or lost keys** (2026-10-25). An unknown key, preset or
  colour falls back to the default and says so at attach, on reload and in `monarch config check`.
- **`terminal.env` cannot override `TERM` or `TERM_PROGRAM`** (2026-11-06). Monarch's own variables
  are set after the user's.

## Delivery

- **Package `monarch-mux`, binary `monarch`; crates `monarch-mux-*` with `[lib] name` kept as
  `monarch_core` etc.** (2026-10-23). `monarch` and `monarch-core` were taken on crates.io.
- **Licence MIT OR Apache-2.0** (2026-10-23). The Rust convention, and it makes vendoring Herdr's
  Apache-2.0 manifests clean.
- **Trunk-based, release only by merging release-plz's PR** (2026-10-23). `main` is protected and
  squash-only; PR titles are conventional commits. Releases happen only on the user's explicit ask;
  crates.io publishing stays off (`git_only`).
- **The README tells Homebrew users to tap and trust first** (2026-11-06). Homebrew 7 refuses
  untrusted taps, and a bare `brew install monarch` installs an unrelated cask.
- **README images come from the real binary** (2026-11-05). `scripts/capture_readme.py` drives it in
  a PTY with scripted agents reporting through the real hook path; the captions say what was
  scripted or added.
