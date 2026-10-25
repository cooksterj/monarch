# Monarch — an agent & terminal organizer (Rust TUI)

## Context

The user runs several AI coding agents (Claude Code, Codex, shells) through Herdr and finds it clunky. Herdr is a serious incumbent (Rust, 45k stars, Apache-2.0, 22 agent-detection manifests, socket API, plugins), so Monarch does not win by matching features — it wins on **user experience**, under one hard constraint the user set: **UX is paramount, but the terminal experience must never be degraded.** Anything running inside a Monarch pane must behave exactly as it would in a bare terminal (keys, mouse, paste, colors, queries, latency).

Decisions already made with the user:

| Decision | Choice | Why |
|---|---|---|
| Language | Rust | PTY/VT work is throughput- and latency-bound; single static binary; `cargo` installed (rustc 1.93.1). Python rejected: `pyte` is slow/incomplete for full-screen agent TUIs and distribution is fragile. |
| Form | TUI (runs in your terminal, works over SSH), tmux-style detachable server | What the user asked for. |
| Key model | **Prefix-only (`Ctrl-b` default)**, fully rebindable | Zero key theft from inner apps — the choice most consistent with "don't degrade the terminal". Discoverability recovers the second keystroke (see UX). |
| Detection | Herdr-manifest-compatible engine; vendor Herdr's Apache-2.0 manifests with attribution | ~20 agents detected on day one; hooks remain the authoritative signal. |
| Platforms v1 | macOS + Linux | Unix sockets + PTYs; Windows later behind the same traits. |
| Naming | crates.io package `monarch-mux`, binary `monarch` | `monarch` is taken on crates.io. Repo, brew formula and installer stay `monarch`. |
| Terminal engine | `alacritty_terminal` 0.26 by default, behind an `Engine` trait | libghostty-vt (Herdr's engine) needs **Zig 0.17 + network at build time**, which breaks `cargo install`. Alacritty fidelity is proven; ghostty can be an opt-in feature later. |
| Repo | greenfield, not yet `git init`; installable from day one | User requirement. |

Research sources (verified 2026-10-23): Herdr repo/docs (`Cargo.toml`, `AGENTS.md`, `src/detect/manifests/*.toml`, keyboard/socket/config docs), Claude Code hooks reference, Codex config reference, crates.io metadata, HN thread on Herdr.

---

## 1. Product thesis & UX design

**One-line:** tmux-grade reliability plus an *attention-first cockpit* for agents.

### 1.1 Where Herdr feels clunky → what Monarch does instead

| Herdr pain (user + HN reports) | Monarch answer |
|---|---|
| Hard to see which agent needs you; "what is it doing?" | **Attention Inbox** + per-agent **live activity line** fed by hooks (`PreToolUse` → "Edit src/x.rs", `Stop` → `last_assistant_message` summary). |
| Navigating panes/tabs/workspaces is a chore | tmux-compatible prefix bindings where tmux has one, mnemonic otherwise; **which-key popup**, **sticky resize/focus modes**, **command palette** (`prefix :`), **go-to picker** with state filters, mouse everywhere. |
| No project/worktree organization | Panes are **auto-grouped by git repo → worktree** (derived from cwd, nothing to set up); `monarch new` creates worktree + agent in one step; **Review pane** shows the worktree diff. |
| Notifications are noisy or missed | Quiet-by-default rules: never for the focused pane, batched bursts, per-agent/state toggles; delivered as toast **and** forwarded to the host terminal (OSC 9/99/777) so desktop notifications work over SSH. |
| Input lag; selection cleared by redraws; keys "all over the place" | The **Terminal Fidelity Contract** (§2) with measurable guarantees and tests. |

### 1.2 Screen anatomy

```
┌ main ▸ [1:monarch] [2:api ●] [3:logs]            ◆ 2 blocked · ✓ 1 done   mbp  14:02 ┐
│ ▸ Inbox (3)                     │                                                   │
│ ▾ monarch  main  ±12            │           pane grid (ratatui layout tree)         │
│   ● claude   Working  Edit main.rs 0:42                                             │
│   ◆ codex    Blocked  "Allow command? cargo test"                                   │
│ ▾ api  feat/auth  ±3            │                                                   │
│   ✓ claude   Done  "Added JWT middleware…" 3m                                       │
│ Scratch                         │                                                   │
│   $ zsh                         │                                                   │
├ prefix hints appear here when Ctrl-b is held (which-key) ───────────────────────────┤
```

- **Status bar (top):** session, tabs with attention dots, right: attention counter, host, clock. Widgets configurable.
- **Sidebar (left, `prefix t` toggles, drag to resize, auto-hides < 100 cols):** Inbox header, then Project → Worktree → panes. Row = state glyph+color · label · activity line · time-in-state. Collapsible groups. Click to focus.
- **Pane grid:** binary split tree with named layouts (`even-h`, `even-v`, `main-vertical`, `tiled`), zoom, per-tab persisted.
- **Hint line (bottom):** context hints; becomes the which-key panel 300 ms after prefix is pressed (no flicker for fast users).
- **Overlays:** command palette, go-to picker, inbox, review pane, help, confirmations — one modal language (title, body, footer keys, `Esc` closes).

### 1.3 Attention Inbox (`prefix a`)

Items: **Blocked** (prompt excerpt taken from the detector's blocker region, e.g. the permission question and its options), **Done** (first line of `last_assistant_message`, time since), **Error/Exited**. Actions: `Enter` jump · `y`/`n`/digits/`Esc` send a quick answer *without leaving the inbox* (per-agent mapping in config; raw key passthrough always available) · `s` snooze · `Tab` next item. `prefix Tab` cycles blocked agents without opening the inbox.

### 1.4 Keybindings (default `monarch` preset; `tmux` and `herdr` presets ship too)

Prefix `Ctrl-b`. Repeatable keys (resize/focus) don't need the prefix again within 500 ms.

| Area | Keys |
|---|---|
| Panes | `%`/`"` split (tmux) also `|`/`-` · `h j k l`/arrows focus · `z` zoom · `x` close (confirms if a process is running) · `r` sticky resize · `{ }` swap · `Space` next layout · `!` break to tab |
| Tabs | `c` new · `n`/`p` · `1-9` · `,` rename · `&` close · `w` picker |
| Agents | `a` inbox · `Tab` next blocked · `A` launch agent (claude/codex/custom) · `i` agent info & detection evidence · `g` go-to picker (filters `b/w/i/d`) |
| Projects | `s` project/worktree picker · `N` new worktree wizard · `v` review pane |
| Session | `d` detach · `:` palette · `?` help · `[` copy mode (vi keys, `/` search) · `]` paste · `m` mouse passthrough toggle · `t` sidebar · `R` reload config |

Mouse: click focus · drag borders · wheel scroll (scrollback, or forwarded if the app wants the mouse) · drag-select copies (Monarch selection in grid coordinates, survives redraws) · double/triple click · right-click context menu · **Shift+drag always falls through to the host terminal's native selection**.

### 1.5 First run, setup, config

- `monarch` with no config → welcome overlay: detects host terminal, probes true color / OSC 52 / Alt delivery, shows the six keys you need, offers `monarch setup`.
- `monarch setup` (idempotent, backs up, `--dry-run`, `--uninstall`): merges hooks into `~/.claude/settings.json` (`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Notification`, `Stop`, `StopFailure`, `SessionEnd` → `monarch hook claude`, 5 s timeout); for Codex adds `notify = ["monarch","hook","codex-notify"]`, `[hooks]` with `features.hooks = true`, and `tui.notification_method = "osc9"` to `~/.codex/config.toml` via `toml_edit`; writes default `~/.config/monarch/config.toml`. Hooks exit immediately when `MONARCH_PANE` is unset, so a global install is safe.
- `monarch doctor` re-runs checks and prints fixes.
- Config `~/.config/monarch/config.toml`: `[keys]`, `[ui]`, `[theme]`, `[notifications]`, `[agents.<id>]` (command, args, quick_answers, resume template), `[projects]` (worktree root), `[terminal]` (shell, scrollback 10k, env), `[session]`. Bad values → toast + safe default, never a crash. Hot reload.
- Themes: semantic tokens; `monarch-dark/light`, catppuccin, tokyonight, gruvbox; auto light/dark from host background (OSC 11 probe); `NO_COLOR` and 256-color fallbacks.

---

## 2. Terminal Fidelity Contract (the "do not degrade" guarantees)

Each guarantee has an owner module and a test (§8). Shipping M1 means G1–G12 pass.

| # | Guarantee | How |
|---|---|---|
| G1 | Keys reach the focused pane **byte-exact** | The client reads `/dev/tty` raw on a dedicated std thread and writes bytes straight to the socket — never through the render loop, never through crossterm's event parser (which normalises and drops sequences). The prefix is a single C0 byte (`Ctrl-b` = `0x02`), so recognising it needs no look-ahead and **no ESC timer**; in kitty mode it arrives as `CSI 98;5u`, still a complete sequence. Everything else — CSI-u/kitty encodings, Alt-, F- and Ctrl- keys, a lone `ESC` — is forwarded unchanged and instantly. Paste bodies are never scanned. |
| G2 | Kitty keyboard protocol works (Claude Code's Shift+Enter) | `term::Config.kitty_keyboard = true` (mandatory — otherwise alacritty ignores pushes). Host: probe `CSI ? u` at startup; one `CSI > 0 u` push; on focus/mode change set flags **absolutely** with `CSI = <bits>;1 u` (never per-pane push/pop — stack drift is the classic bug); `CSI < u` on exit. Non-kitty hosts get legacy bytes, as in a bare terminal. |
| G3 | Mouse behaves like a bare terminal | Mirror pane mouse modes (`MOUSE_*`, `SGR_MOUSE`, `UTF8_MOUSE`) to host; translate coordinates to pane-local; wheel → scrollback when no mouse mode and not alt-screen; `ALTERNATE_SCROLL` → arrow keys. |
| G4 | Paste is intact | Mirror `BRACKETED_PASTE`; forward paste atomically; large pastes not chunk-split mid-sequence. |
| G5 | Terminal queries answered correctly | DA1/DA2/XTVERSION/DSR via alacritty `Event::PtyWrite` (ordered through the pane's writer channel); OSC 10/11/12 answered **without a round trip** from `HostInfo{fg,bg,cursor,cell_w,cell_h,sync,kitty}` probed at attach/FocusGained (`Event::ColorRequest` closure; pane overrides via `term.colors()` win) — Claude Code's theme detection just works; `TextAreaSizeRequest` from host cell-pixel size (`CSI 16 t`); OSC 52 store → `OSC 52` to every attached host (`Osc52::OnlyCopy`, load disabled). |
| G6 | Focus events | `FOCUS_IN_OUT` mirrored; synthetic focus in/out on pane-focus change. |
| G7 | Title and cursor | Focused pane title → host title (format configurable); DECSCUSR shape/blink and cursor visibility forwarded. |
| G8 | No tearing, no lag | Every frame wrapped in DEC 2026 synchronized output when DECRQM says the host supports it; ratatui diff draw; render coalescing (16 ms budget, configurable); the key→PTY-write path never touches the render loop (p99 < 2 ms local, measured in spike 4). Inner-app sync (DEC 2026 from the pane) is honoured by vte, and the reader thread drives `Processor::stop_sync` from a `poll(2)` timeout so an app dying mid-frame can't freeze its pane. |
| G9 | Scrollback & reflow | ≥ 10k lines/pane (configurable), reflow on resize, search, copy mode. |
| G10 | Selection survives output | Selection anchored to grid cells; Shift+drag and `prefix m` passthrough give the host terminal's native selection. |
| G11 | Environment is honest | `TERM=xterm-256color` (alacritty_terminal is xterm-class; terminfo exists everywhere incl. over SSH), `COLORTERM=truecolor`, **`TERM_PROGRAM=monarch`** + `TERM_PROGRAM_VERSION` — Monarch never impersonates the host terminal (an app told it's in iTerm2 would also emit iTerm image/escape protocols Monarch can't honour, which is exactly the degradation we're avoiding). Consequence: Claude Code won't send OSC 9 to us (it only does so for Ghostty/kitty/iTerm2), so hooks are the primary state signal (§3.3). Add `MONARCH_PANE`, `MONARCH_SESSION`, `MONARCH_SOCKET`; unset `TMUX*`, `ZELLIJ*`, `STY`. |
| G12 | Process semantics | SIGWINCH on resize, SIGHUP on close, exit status shown, children reaped; wide/emoji/grapheme widths consistent (pin `unicode-width` to alacritty's). |
| G13 | Background panes stay correct, not expensive | Hidden panes keep parsing (so state detection and scrollback are right) but trigger no presentation work; background resizes debounced. |
| — | Known v1 limits (documented) | No kitty-graphics/sixel passthrough; no Windows. |

Compatibility matrix (manual checklist each release): Ghostty, iTerm2, kitty, WezTerm, Alacritty, Terminal.app; local and over SSH/mosh.

---

## 3. Architecture

Client/server like tmux: the **server** owns PTYs, emulators, agent state and persistence; the **TUI client** is thin and multiple clients can attach to one session.

```
monarch (bin)  ── clap ──►  monarch-cli
   │  attach/new/ls/kill/hook/setup/doctor/api/server
   ▼
monarch-tui  ◄── attach stream (binary frames) ──►  monarch-server (tokio daemon)
                 public JSON-lines API (CLI, hooks, agents) ─┘   │
                                                                 ├─ monarch-term   (Engine trait, alacritty backend, OSC interceptor, selection, search)
                                                                 ├─ monarch-detect (manifest schema + evaluator + process probe)
                                                                 └─ monarch-core   (domain types, config, keymap, theme — no IO)
                                           monarch-proto (versioned wire types shared by all)
```

### 3.1 Cargo workspace (root is the installable package)

```
Cargo.toml              [workspace] + [package] name="monarch-mux", [[bin]] name="monarch"
src/main.rs             thin → monarch_cli::run()
crates/monarch-core     Project/Worktree/Tab/Pane/Agent/AgentState ids & types, Config (serde+toml), Keymap, Theme
crates/monarch-proto    JSON API types (serde, schemars) + attach-stream frames; ClientMsg/ServerMsg with version field
crates/monarch-term     Engine trait; AlacrittyEngine; OscTap (OSC 9/99/777/0/2/52 + OSC 9;4 progress); Selection; Search
crates/monarch-detect   Herdr-compatible manifest loader/evaluator; foreground-process probe; `explain`
crates/monarch-server   Session, PaneRuntime (portable-pty), StateEngine, HooksIngest, Notifier, Persist, ApiServer, AttachServer
crates/monarch-tui      raw input + prefix parser, keymap dispatch, AppState (pure) + render (pure), widgets, views
crates/monarch-cli      clap commands; `hook` subcommands are tiny and fast (connect socket, write, exit)
manifests/              vendored Herdr manifests (Apache-2.0) + ATTRIBUTION.md; embedded with include_dir!
```

Crates: `ratatui` 0.30, `crossterm` 0.29 (commands only — raw mode, alt screen, synchronized update, cursor style, title; input is raw `/dev/tty`), `alacritty_terminal` 0.26 with `default-features = false` (+ its `vte` 0.15 with `std,ansi`), `portable-pty` 0.9, `tokio` + `tokio-util` (codec), `postcard` (attach stream; bincode is unmaintained), `serde`/`serde_json`/`toml`/`toml_edit`, `schemars`, `regex`, `memchr`, `nucleo` (fuzzy), `unicode-width` (pinned to alacritty's), `tracing`, `clap`/`clap_complete`, `directories`, `libproc` (macOS cwd). Git operations shell out to `git` (worktrees, status, diff) rather than linking libgit2 — fewer build deps, honours user git config.

Borrowed from Herdr's `AGENTS.md` as project rules (recorded in `CLAUDE.md`): state is pure data separate from runtime; render is pure (`compute_view` then `render(&AppState)`); detection reads a screen snapshot, never the parser; anything on the *per-byte × panes × clients* path is treated as multiplicative; terminal-core locks held briefly.

### 3.2 Domain model

- **Session** (named, default `main`) → **Tabs** → split tree of **Panes**.
- **Project / Worktree** are *derived* from each pane's cwd (`git rev-parse --show-toplevel`, `git worktree list --porcelain`, refreshed on `CwdChanged`/OSC 7/periodic) — the sidebar groups by them with no setup. `monarch new --repo . --worktree feat-x --agent claude` creates the worktree under `[projects].worktree_root` and starts the agent there.
- **Agent** = a pane with a detected or declared agent (`claude`, `codex`, custom). Fields: kind, state, activity line, last summary, native session id, resume argv, state history.
- **AgentState**: `Working`, `Blocked` (needs input), `Idle`, `Done` (Idle reached via Stop/`agent_completed`; carries summary; decays to Idle when the user focuses the pane), `Error`, `Exited`, `Unknown` (plain shells).

### 3.3 Agent state engine (fusion of four signal sources)

1. **Hooks (authoritative).** `monarch hook claude` / `monarch hook codex` read the hook JSON on stdin and post `agent.report` to the socket using `MONARCH_PANE` from the inherited env. Claude Code events used: `SessionStart` (session_id → resume), `UserPromptSubmit` → Working, `PreToolUse`/`PostToolUse` → activity line, `Notification` (`permission_prompt`, `idle_prompt`, `agent_needs_input`, `elicitation_dialog`) → Blocked, `Stop` (`last_assistant_message`, `background_tasks`) → Done or Working-in-background, `StopFailure` → Error, `SessionEnd`. Codex: `notify` (`agent-turn-complete`) plus the same hook events under `[hooks]`.
2. **OSC notifications** emitted by the agent inside the pane (OSC 9 / 99 / 777, OSC 9;4 progress, OSC 7 cwd, OSC 133 prompt marks): intercepted by `OscTap`, mapped to Blocked/Done, and *also forwarded* to the host. Matters most for Codex: its `notify` hook fires only on `agent-turn-complete`, so `monarch setup` pins `tui.notification_method = "osc9"` and `tui.notifications = true` so **approval prompts** reach the tap. Claude Code does not emit OSC 9 under `TERM_PROGRAM=monarch` (see G11); its hooks cover everything, with `preferredNotifChannel: terminal_bell` + BEL as an optional belt-and-braces fallback.
3. **Screen manifests** (fallback and for hook-less agents): Herdr-compatible TOML. Must support regions `bottom_non_empty_lines(n)`, `whole_recent`, `osc_title`, `osc_progress`, `after_last_prompt_marker`, `after_last_horizontal_rule`, `whole_recent_without_current_prompt_marker`, `top_non_empty_lines(n)`, `prompt_box_body`, `last_non_empty_above_prompt_box`, `before_current_prompt_marker`; gates `contains`, `regex`, `line_regex`, `any`, `all`, `not`; rule fields `id/state/priority/visible_*/skip_state_update`; header `id/version/min_engine_version/aliases`. Manifest chosen by foreground process name (matched against `id`/`aliases`). Evaluated on a debounced snapshot of the *bottom buffer*, never the user-scrolled viewport.
4. **Quiescence** (weakest): output silence timers only break ties.

Fusion: freshest highest-tier signal wins; a hook state holds until contradicted by a hook or a `visible_blocker` screen match; every transition is an event (bus → sidebar, inbox, notifier, API subscribers). `monarch agent explain <pane>` prints the evidence. Overrides live in `~/.config/monarch/agent-detection/<id>.toml` and hot-reload.

### 3.4 Public API & CLI

- JSON-lines over a Unix socket at `$XDG_RUNTIME_DIR/monarch/<session>.sock` (fallback `~/.local/state/monarch/run/`, dir 0700, socket 0600). Request `{id, method, params}` → `{id, result|error}`; `events.subscribe` keeps the line open; lagging subscribers get `events_lost` and resync via `session.snapshot`.
- Namespaces: `session.*`, `tab.*`, `pane.*` (split/run/send_keys/read/resize/zoom), `agent.*` (list/report/wait/prompt/explain), `project.*`, `worktree.*` (create/open/remove), `layout.*`, `notification.show`, `events.*`. Schema from `schemars` → `monarch api schema`.
- CLI mirrors the API (`monarch pane run w1:p2 "cargo test"`, `monarch agent wait w1:p1 --until blocked`). A `skills/monarch/SKILL.md` teaches agents to use it (M6).

### 3.5 Persistence & restore

`~/.local/state/monarch/<session>/state.json` (XDG on both OSes for predictability over SSH; atomic rename; debounced 500 ms on layout/title/cwd change, every 30 s, and on shutdown): tabs, split tree (ratios, zoom, focus), per pane: id, label, kind ∈ {shell, claude, codex, custom}, argv + env overrides, start cwd and *live* cwd (macOS `libproc` / Linux `/proc/<pid>/cwd` / OSC 7 from the tap), agent session id, last state, last 2k text lines shown greyed until the restored process writes. Claude Code: Monarch launches agent panes with **`claude --session-id <uuid>`** (verified flag; must be a UUID) so restore is simply `claude --resume <uuid>`. Codex: `codex resume` exists; resume-by-id semantics are verified in spike 6. Other panes get a shell at the old cwd with a one-line "restored" banner. Processes, termios state, alt-screen contents and unsent input cannot survive a server restart (same as Herdr).

### 3.6 Core terminal layer (APIs verified against alacritty_terminal 0.26 / vte 0.15 / portable-pty 0.9)

Full design with rationale: `/Users/jonathancook/.claude/plans/this-repository-will-be-fancy-koala-agent-acore-arch-3e1aab616757b4f1.md`. The decisions:

**Headless emulator (`monarch-term`).**
- `Term::new(term::Config { scrolling_history, kitty_keyboard: true, osc52: Osc52::OnlyCopy, .. }, &TermSize::new(cols, lines), PaneListener)`; `PaneListener: EventListener` only enqueues `(PaneId, Event)` on an unbounded channel (events fire synchronously inside `advance` under the lock — never do work there). `Event` closures are `Arc<dyn Fn + Send + Sync>`, so the term lives in `Arc<FairMutex<Term<PaneListener>>>`.
- Feed: `vte::ansi::Processor<StdSyncHandler>::advance(&mut *term, bytes)`; after each call read `processor.sync_timeout()` and use it as the `poll(2)` timeout; on expiry `stop_sync`.
- Read: `term.renderable_content()` (cells, cursor, `display_offset`, colors, `TermMode`), `term.damage()` → `TermDamage::{Full, Partial(line bounds)}` + `reset_damage()` once per frame (line-level dirtiness is what makes grid deltas cheap). Scrollback via `grid().history_size()` / `scroll_display(Scroll::…)`. Resize: `term.resize(TermSize)` **before** `master.resize(PtySize)`. Selection: `Selection::new(SelectionType::{Simple,Block,Semantic,Lines}, point, side)` + `selection_to_string()`; mouse → `viewport_to_point`.
- **`OscTap`** sits in front of the parser: a four-state scanner (Ground / Esc / Osc / OscEsc) that `memchr`s for `ESC` in Ground so plain text costs ~0; emits `Osc { num, payload }` for an allowlist (0/2 title, 7 cwd, 9/99/777 notifications incl. `9;4` progress, 52, 133, 7501); 4 KiB payload cap. A second `vte::Parser` is the test oracle, not the production path. Scrollback default 5k lines for agent panes, 10k for shells (memory is tens of MB per pane at 10k × wide).
- `Engine` trait wraps all of the above so a ghostty backend can be added as an opt-in feature later.

**Threads.** Two std threads per pane: *reader* (`poll(2)` on the master fd → `read` ≤ 64 KiB → `OscTap::scan` → `lock_unfair` → `advance` → dirty + notify) and *writer* (one bounded channel carrying user input **and** `PtyWrite` replies, so order is preserved). A tokio multi-thread runtime owns sockets, API, the frame coalescer, state engine and persistence and never does PTY I/O. portable-pty maps `EIO` to `Ok(0)` = child exit.

**Attach stream = server-side emulation + cell-grid deltas** (tmux/zellij model; raw-byte forwarding rejected: the server must emulate anyway for state/restore/`pane.read`, raw attach needs a Term→escape re-serialiser plus mode replay, and agent TUIs emit far more bytes than net cell change). `postcard` frames with a big-endian `u32` length and a 16 MiB limit (bincode 2, the first choice, became unmaintained — RUSTSEC-2025-0141). S→C: `Layout{gen,…}`, `Snapshot{pane,gen,seq,rows,cursor,offset,mode_bits,cursor_style}`, `Delta{pane,gen,seq,rows:[RowDelta{line, runs:[Run{style_id,text,cells}]}],…}`, `DefineStyle` (per-connection 4096-entry cache), `HostWrite{bytes}` (OSC 52, BEL), `Event{json}`. C→S: `Input{pane,bytes}`, `Paste`, `Resize`, `HostInfo`, `Focus`, `Scroll`, `Mouse{pane,x,y,kind}`, `Resync`, `Command{json}`.

**Coalescer** (one task per session): on notify, render now if idle ≥ budget else at `last + budget` (16 ms). For each *visible* dirty pane: fair `lock()` → copy only damaged rows into a shadow grid → `reset_damage()` → unlock → diff shadow → `Delta`. Lock time scales with damaged rows, not output volume. Hidden panes accumulate damage and are skipped; `Snapshot` on reveal. The state detector runs ≤ 4 Hz on a text copy of the last N lines.

**Backpressure & resize.** Per client an input lane (bounded 256; blocks only that socket, never a PTY) and a render lane (~64 frames / 4 MiB); overflow drops that client's queued deltas and marks it `resync` → `Snapshot`s of its visible panes next tick. Per-pane monotonic `seq`; a gap triggers `Resync{pane}`. Resize: latest client wins (tmux `window-size latest`); server resizes Term then PTY, bumps `gen`, sends `Layout` + `Snapshot`s; clients discard stale-`gen` deltas.

**Socket & handshake.** One server per session at `$XDG_RUNTIME_DIR/monarch/<session>.sock`, else `$TMPDIR/monarch-<uid>/`, else `/tmp/monarch-<uid>/` (dir 0700; stale socket unlinked on failed connect). First line JSON `{"monarch":1,"proto":"json"|"attach","version":…}` → `{"ok":true,…}`; `json` continues as JSON-RPC 2.0 lines (§3.4), `attach` switches to binary frames.

**Host-side mirroring (`monarch-tui/host_modes.rs`).** Table-driven from the focused pane's `TermMode`: kitty flags (G2), bracketed paste always on at the host and stripped unless the pane set it, focus reporting always on and forwarded only if set (synthetic `O`/`I` on pane switch), mouse re-encoded in the pane's encoding with pane-local coordinates, `ALTERNATE_SCROLL` → 3× `CSI A/B` (`SS3` under `APP_CURSOR`), DECSCUSR + visibility, title via OSC 2. Mouse policy: `mouse = auto` (default: capture, Shift+drag falls through, `prefix m` toggles) or `mouse = pane` (capture only while the focused pane requests the mouse, so native drag works everywhere else). Idle panes are never redrawn (no ticking clock in cells), which is what keeps host-native selections alive.

**Unverified, resolved in spikes:** Codex resume-by-id semantics; how Claude Code probes OSC 7501 (the tap captures it regardless).

---

### 3.7 Code organization & Rust conventions (recorded in `CLAUDE.md`, enforced by CI)

**Files and modules — "one type per file".**
- Rust has no classes; the unit is a **module**. Convention: **one primary `struct`/`enum` per file, with its `impl` blocks and trait impls, file named after the type in snake_case** (`PaneRuntime` → `pane_runtime.rs`, `AgentState` → `agent_state.rs`). Small private helper types may live beside their owner; anything `pub` gets its own file.
- Module style is Rust-2018: `foo.rs` declares `mod bar;` and re-exports; children live in `foo/bar.rs`. No `mod.rs`, no `utils.rs`/`helpers.rs` grab-bags, no file over ~400 lines (split `impl` blocks across files in the same module when needed — Rust allows it).
- `lib.rs` of each crate contains only `mod` declarations, `pub use` re-exports and crate docs. Visibility defaults to `pub(crate)`; each crate's public surface is deliberate and documented.
- Every file starts with `//!` docs stating its single responsibility. Every `pub` item has `///` docs (`#![warn(missing_docs)]` on library crates).

**Small files (hard limits, checked in CI by a `just lint-size` script).** Target ≤ 200 lines, hard cap 300 for source files (tests excluded); a type that outgrows its file is split by responsibility (e.g. `impl StateEngine` fusion logic vs. its ingest adapters), never into a second type in the same file. Functions ≤ 40 lines, ≤ 5 parameters (use a params struct beyond that).

**DRY.**
- One source of truth per concept: IDs, `AgentState`, `Chord`, `Theme` tokens and wire types are defined once in `core`/`proto` and imported everywhere; no parallel enums or hand-written conversions where `From`/`Into`/`TryFrom` impls belong.
- Shared logic goes in a named helper type or trait default method, never copy-pasted; three similar `match` arms become a table or a generic. Repeated per-namespace API plumbing (`pane.*`, `tab.*`, …) is generated by one small declarative `macro_rules!` (`define_methods!`) rather than duplicated by hand; macros are allowed only for this kind of boilerplate and must live in one file with tests.
- Rendering styles come from `Theme` tokens only — no literal colours in widgets. Config defaults are declared once (`Default` impls) and reused by docs (`monarch config default`) and tests.

**Inheritance-style reuse, the Rust way (no class inheritance exists; these are the required substitutes).**
- *Shared behaviour* → traits with **default methods**: `NotificationChannel::should_deliver` has one default used by every channel; `Check::run` for doctor checks; `Handler::validate` for API handlers.
- *Polymorphism* → trait objects or generics: `Box<dyn Engine>`, `Vec<Box<dyn NotificationChannel>>`, `&dyn SidebarRow`.
- *Shared fields* → composition: `PaneCommon` embedded in agent and shell panes, exposed through a `PaneInfo` trait so callers never reach into fields twice.
- *Closed hierarchies* → enums with per-variant data and exhaustive `match` (`Signal`, `Action`, `ServerMsg`), so adding a variant is a compile error everywhere it isn't handled.
- *Layered contracts* → supertraits (`trait AgentDriver: Resumable + Detectable`).
- `Deref` delegation is allowed only for pure wrappers; never to fake an is-a relationship.

**Design.**
- Composition and traits, not inheritance (see above). Seams are traits (`Engine`, `NotificationChannel`, `ProcessProbe`, `Clock`, `GitRunner`) so everything above the PTY is testable without a PTY.
- Data vs behaviour: pure data structs with `pub` fields (`AppState`, `Config`, proto types); behaviour on runtime types (`PaneRuntime`, `StateEngine`). Render is pure: `compute_view(&AppState) -> ViewModel`, `render(&ViewModel, &mut Frame)`.
- Newtypes for identifiers (`PaneId(u32)`), enums instead of stringly/boolean flags, `#[non_exhaustive]` on wire enums, builders for anything with > 3 optional inputs, `Default` derived where sensible.
- Errors: `thiserror` enums per library crate; `anyhow` only in `monarch-cli`/`main.rs`. No `unwrap()` outside tests; `expect("invariant: …")` only for true invariants. Hot paths never allocate per byte.
- Concurrency: std threads for blocking PTY I/O, tokio only for sockets/timers; bounded channels everywhere; no `Arc<Mutex<_>>` sprawl — the single `FairMutex<Term>` per pane is the exception and locks are held for row copies only.
- Platform code isolated in `platform/{macos,linux}.rs` behind traits; no `#[cfg(target_os)]` elsewhere. `#![forbid(unsafe_code)]` on every crate except the platform modules.
- Crate dependency graph is acyclic: `core` (no IO) ← `proto` ← `term`/`detect` ← `server`/`tui` ← `cli` ← bin.

**Tests (required, not optional).**
- Every file that defines a `pub` type ships a `#[cfg(test)] mod tests` in the same file covering its public behaviour, including at least one failure/edge case; a PR adding a type without tests fails review.
- Test doubles are built from the seams: `FakeEngine`, `FakeGitRunner`, `FakeClock`, `MemoryNotificationChannel`, `AppState::test_new()`, `Session::test_new()`; no test spawns a PTY except the `tests/fidelity/` suite.
- Per crate: `tests/` integration tests; `proptest` for every byte/char parser (`OscTap`, `PrefixParser`, `Chord`, attach codec round-trips); `insta` snapshots for every widget/view at two terminal sizes and light/dark themes; fixture-driven tests for every vendored detection manifest (recorded working/blocked/idle screens); `criterion` benches for parse, coalesce and render with 1/15/30 panes.
- Coverage measured with `cargo llvm-cov`; gate at 80 % lines for `core`, `proto`, `detect`, `term` and 70 % overall, ratcheting upward.

**Quality gates (CI on macOS + Linux).** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (+ a curated `clippy::pedantic` set in `[workspace.lints]`), `cargo nextest`, `cargo deny` (licenses/advisories), MSRV check, file-size lint, coverage gate, benches on a nightly schedule. User-facing config uses `serde_ignored` to *warn* on unknown keys rather than reject them.

## 4. Installability (first-class from M0)

- Repo `github.com/cooksterj/monarch`; license **MIT OR Apache-2.0**.
- Root package `monarch-mux` with full metadata; `cargo install monarch-mux` and `cargo install --git https://github.com/cooksterj/monarch` both work with no extra toolchain (no Zig, no libgit2).
- `cargo-dist` (v0.33.1, active) release workflow: tag `v*` → GitHub Release with tarballs + checksums for macOS arm64/x86_64 and Linux x86_64/aarch64 (musl, static); `curl -fsSL https://github.com/cooksterj/monarch/releases/latest/download/monarch-installer.sh | sh`; Homebrew formula pushed to `cooksterj/homebrew-monarch` (`brew install cooksterj/monarch/monarch`).
- `rust-toolchain.toml` (stable), MSRV 1.88 (raised from 1.86 for the `time` RUSTSEC-2026-0009 fix), `.github/workflows/ci.yml` (fmt, clippy `-D warnings`, nextest on macOS + Linux), `README.md` with all install channels, `LICENSE-MIT` + `LICENSE-APACHE` (dual, as Rust convention — also makes vendoring Herdr's Apache-2.0 manifests clean), `NOTICE`/`manifests/ATTRIBUTION.md`, `CHANGELOG.md`, `CLAUDE.md` with the project rules above.
- `monarch setup` / `monarch doctor` handle post-install configuration (§1.5).

---

## 5. Milestones (each ends with a UX checkpoint the user can try)

**M0 — Scaffold & installability.** `git init`; workspace per §7 with the conventions of §3.7 in `CLAUDE.md`; copy this plan to `docs/plan.md` and the core design to `docs/architecture/terminal-core.md`; CI; `cargo-dist init`; `cargo deny`; `cargo install --path .` yields `monarch --version`; config loading; empty `monarch` prints a help/welcome screen. *Checkpoint: installs cleanly on this Mac.*

**M1 — Terminal core spike (the contract).** One pane, no chrome, built as six gated spikes:
1. *Headless engine* — single-process prototype (portable-pty → `OscTap` → `Processor` → `Term` → ratatui) running `vim`, `htop`, `less`, `claude`, `codex`; parity with a bare terminal on resize/reflow, scrollback, wide/zero-width chars; kill an app mid-frame to exercise the sync-timeout path.
2. *OSC tap* — replay recorded Claude Code/Codex transcripts; tap output equals the `vte::Parser` oracle; `cat` of 200 MB costs < 5 % extra; OSC 11 answered and Claude Code picks the matching theme.
3. *Input fidelity matrix* — pane runs `cat -v`; scripted keys (lone Esc, Alt+x, Ctrl-b, Shift+Enter, arrows under `APP_CURSOR`, F-keys, 1 MB bracketed paste, kitty flags 1 and 1|8) from kitty, Ghostty, iTerm2, Alacritty, diffed against a bare terminal: byte-identical except the prefix; Shift+Enter newlines in `claude`; no Esc delay in `vim`.
4. *Latency & load* — 20 panes (10 × `yes`, 10 idle `claude`), two clients, one over `ssh localhost`: key→`write(2)` p99 < 2 ms, bytes/s per client, CPU; hidden panes ≈ 0 render cost; a stalled client resyncs without affecting others.
5. *Mouse & selection* — wheel in `less`/`vim`, clicks in `htop`, native Shift+drag with capture on, `mouse = pane`, zoom selection, OSC 52 reaching the host.
6. *Detach/restore* — reattach is pixel-identical via `Snapshot`; `kill -9` the server → layout, cwd and `claude --resume <uuid>` restored; determine the Codex resume mechanism.

*Checkpoint: G1–G13 pass; no UI chrome is built until they do.*

**M2 — Multiplexing & navigation.** Tabs, split tree + layouts, zoom, prefix keymap + presets + which-key + repeatable keys, command palette, go-to picker, copy mode + search, mouse (focus/resize/scroll/select/context menu), status bar, sidebar skeleton (grouped by cwd repo/worktree), multi-client attach, config hot reload. *Checkpoint: daily-drivable replacement for tmux.*

**M3 — Agent awareness.** `monarch hook`/`setup`, state engine, OSC-based detection, manifest engine + vendored manifests + `explain`, sidebar states + activity lines, Attention Inbox with quick answers, notifications (toast, host OSC forwarding, bell, command), `agent.*` API + `monarch agent wait`. *Checkpoint: run 4 agents, never miss a prompt, answer one from the inbox.*

**M4 — Projects & worktrees.** `monarch new` worktree wizard, project picker, worktree actions (open/remove), Review pane (file list + colored diff via `git diff --color`, open in `$EDITOR`), dirty/ahead-behind badges. *Checkpoint: start a feature branch + agent in one keystroke sequence, review its diff without leaving Monarch.*

**M5 — Persistence & restore.** State file, crash-safe writes, restore layout, agent resume, `monarch ls/kill`, named sessions. *Checkpoint: kill the server, start it, everything is back where it was.*

**M6 — Polish & release 0.1.** Themes + auto light/dark, first-run welcome + doctor, help/keys overlay, README/docs, `SKILL.md`, performance pass with 20 panes, release via `cargo-dist`, Homebrew tap, crates.io publish.

Explicitly out of v1: Windows, remote SSH machine management, plugins, image protocols, Herdr socket-API compatibility (format similarity is a bonus, not a goal).

---

## 5b. Pause / resume protocol (multi-day implementation)

**Source of truth is the repo, not the conversation.**
- `docs/plan.md` (this file, copied in at M0) ends with a **Progress** checklist: one checkbox per milestone and per M1 spike, plus a "Next up" line and a "Known gaps / decisions pending" list. It is updated in the same commit as the work it describes.
- Commit after every green spike or coherent slice (`feat(term): OscTap scanner with vte oracle tests`), never leave uncommitted work at the end of a session. `cargo nextest` must pass before a commit; if it doesn't, commit on a `wip/` branch and say so in Progress.
- `CLAUDE.md` tells any session to read `docs/plan.md` → Progress first, run `cargo nextest` to confirm the baseline, then continue from "Next up".

**Resuming this session.** From `~/Projects/repos/monarch`: `claude -c` (most recent session here) or `claude -r` (pick from a list). Context is summarized automatically on long runs, so stopping mid-milestone is fine.

**Claude memory.** The project memory directory holds a pointer to the plan, the locked decisions (prefix-only keys, alacritty engine, `monarch-mux`/`monarch`, conventions in §3.7) and the rule "check Progress before doing anything".

**Progress** (maintained in `docs/plan.md` from M0 on)
- [x] M0 scaffold & installability (2026-10-23): workspace + 7 crates, config system + `monarch config path|default|check`, welcome screen (replaced by attach in spike 4), CLAUDE.md conventions, CI (fmt/clippy/lint-size/nextest/msrv/deny), cargo-dist release (shell + Homebrew `cooksterj/homebrew-monarch`, formula `monarch`), licenses
- [x] Repo & delivery (2026-10-23): public `cooksterj/monarch`, trunk-based (protected `main`, squash-only, conventional PR titles), release-plz release PRs → `vX.Y.Z` tag → cargo-dist binaries/installer/Homebrew (crates.io publishing deliberately off until mature: `git_only = true`, versions from tags); Dependabot; MSRV 1.88
- [ ] M1 spikes: 1 engine · 2 OSC tap · 3 input matrix · 4 latency/load · 5 mouse/selection · 6 detach/restore
  - [~] Spike 1 engine: `AlacrittyEngine` + `Terminal` + proto `Screen` + `PaneView` + `PrefixParser` done; prototype `cargo run -p monarch-mux-tui --example single_pane [-- cmd]` passes the automated smoke (`python3 scripts/spike1_smoke.py target/debug/examples/single_pane`). **Remaining: manual parity check by the user** in a real terminal with vim, htop, less, claude, codex (resize/reflow, scrollback, wide chars), and the kill-mid-frame sync-timeout path (handled in the server by `EventLoop`; the single-process prototype still doesn't).
  - [~] Spike 2 OSC tap: `OscTap` matches `vte::Parser` in a property test over random chunkings. Remaining: replay recorded Claude/Codex transcripts; 200 MB `cat` overhead benchmark.
  - [~] Spike 3 input matrix: `HostProbe` (OSC 10/11/12, `CSI 16 t`, DECRQM 2026, `CSI ? u`, DA1 terminator) → proto `HostInfo` → `HostProfile`; `HostModeMirror` (DECCKM, DECKPAM, mouse 1000/1002/1003/1005/1006, 1007 gated on alt screen, kitty `CSI = bits;1u` over one push, DECSCUSR, OSC 2 title, restore on exit); `HostInputFilter` (paste markers / focus events stripped unless the pane enabled them; split markers held only from `ESC [2…`); `PrefixParser` understands kitty-encoded prefix/command keys and swallows their releases; prototype wraps frames in DEC 2026 when supported. Headless matrix `scripts/spike3_input_matrix.py` (in `just smoke` and CI) passes: keys byte-exact incl. lone Esc, Alt, F-keys, UTF-8, 1 MiB paste; modes mirrored and restored. **Remaining: manual matrix in real kitty, Ghostty, iTerm2, Alacritty, Terminal.app** (Shift+Enter in `claude`, no Esc delay in vim, `cat -v` diff vs bare terminal); re-probe on FocusGained.
  - [x] Spike 4 latency & load (automated parts; ssh and real-claude runs are manual): **(1) proto attach messages + codec ✅** (`ServerMsg`/`ClientMsg`, `Snapshot`/`Delta`/`RowDelta`, `Run` + `RowCodec`, `StyleTable`/`StyleCache`, `LayoutMsg`, `FrameCodec` = postcard in `u32`-length frames, blocking + tokio); **(2) term damage → row changes ✅** (`Engine::rows`/`pane_state`, `Terminal::capture` copies only damaged rows under the lock, `ShadowGrid::apply` diffs outside it → `GridChange`); **(3) server ✅** (`PaneRuntime` = PTY reader/writer std threads around `PaneShared`; `SessionState` behind one mutex with panes via the `PaneIo` seam, render logic tested on fake panes; `FrameCoalescer` 16 ms budget; per-client `ClientHandle` with a 64-frame lane, overflow → resync snapshots; `EventLoop` for titles/bells/exits and stalled DEC 2026 frames via a timer — closes the spike-1 sync-timeout gap; `AttachConnection` JSON hello then frames; `Server` binds `SocketPath` 0600, refuses a live duplicate, exits when the last pane does; end-to-end test over a real PTY); **(4) attach client ✅** (`monarch` / `monarch attach [-s name]` auto-starts `monarch server --daemon` (own session via `setsid`, stderr to `<socket>.log`) and attaches; `AttachApp` main loop applies frames to a pure `AttachState` (`PaneReplica` per pane, resync on gaps), mirrors the focused pane's modes/title, draws in DEC 2026; raw input on its own thread → `HostProbe` → `PrefixParser` → `HostInputFilter` → `ClientMsg::Input` without touching the render loop; keys typed before the first layout wait for focus; running `monarch` inside a pane is refused (it would attach to itself and loop every key back into its own pane), as tmux refuses nesting; `Ctrl-b d` detaches; a one-row status line (`StatusLine`: `monarch │ session │ pane title`, prefix hint, `PREFIX` while a command key is awaited; reverse video until theme tokens exist) and the `[ui].window_title` host title make it obvious you are inside Monarch; headless `scripts/spike4_attach_smoke.py` (in `just smoke` + CI): attach, type, detach, reattach with the screen intact, exit ends the session); **(5) bench ✅** (`just bench-spike4`, results in `docs/bench/spike4.md`): 22 panes (probe, churn, 10 × `yes`, 10 idle), 2 clients: key→pane p99 **0.20–0.35 ms** (budget 2 ms) even while both clients render a full-speed pane; hidden panes send 0 B/s; a stalled client resyncs without affecting the other. Remaining manual: second client over `ssh localhost`, ten real idle `claude` panes (`--agent claude`)
  - [x] Spike 5 mouse & selection (automated parts; real-terminal check is manual): **(1) routing ✅**. Host capture follows `[ui] mouse` (`auto` always tracks drags, `pane` only while the pane tracks, `off` never), always in SGR. `MouseExtractor` cuts SGR reports out of the input stream (paste-aware; holds only from `ESC [ <`). `MouseRouter` (pure) sends tracking panes their own encoding (SGR / UTF-8 / X10 via `MouseEncoder`) with pane-local coordinates, and a press keeps its drag/release. Without tracking, the wheel scrolls history on the primary screen (`ClientMsg::Scroll`, 3 lines/notch) and becomes arrow keys on the alternate screen under 1007, as in a bare terminal. Typing while scrolled back returns to live output. Headless `scripts/spike5_mouse_smoke.py` (in `just smoke` + CI). **(2) selection ✅**:
  - In a non-tracking pane (or with Shift over a tracking one), left drag selects, double-click selects a word (paths and URLs stay whole) and triple-click a line.
  - The `Selection` is anchored to absolute grid lines, so the highlight follows its text as output scrolls. It's drawn in reverse video and cleared by typing.
  - On release the text goes to the host via OSC 52, plus `pbcopy` / `wl-copy` / `xclip` when the client isn't running over SSH (iTerm2 ignores OSC 52 by default). The status line shows "copied N characters" for 2 s.
  - The smoke test covers double-click and drag copies.

  **(3) pane copies ✅:** a program's OSC 52 store (vim `"+y`, nvim, tmux) becomes `ServerMsg::Clipboard`, sent only for visible panes (a background program can't overwrite the clipboard) and only up to 1 MiB. The client copies it through the same OSC 52 + local-clipboard path; reading the clipboard is never offered. **(4) `prefix m` ✅:** hands the mouse to the host terminal (native selection, its menus) with a status-line notice, and takes it back.

  **Remaining (manual):** try in real terminals: wheel in `less`/`vim`, clicks in `htop`, drag/double/triple-click copy, `"+y` in vim, `prefix m`. Shift+drag (Option+drag in iTerm2) should still give native selection.

  **Selection limits:** only lines in view are copied; there's no auto-scroll while dragging past an edge; soft-wrapped lines copy with a newline at the wrap (the proto has no wrap flag yet).
  - [x] Spike 6 detach & restore (automated parts; a real `claude` resume is manual): **(1) reattach is identical ✅**: `crates/monarch-cli/tests/reattach.rs` runs a real server with a pane drawing colours, attributes, wide and combining characters, cursor addressing, a title and ongoing output; a client that stayed attached (deltas) and one that detached and came back (snapshot) end with equal `Screen`s and equal drawn frames. **Codex resume ✅ (researched)**: Codex can't be started with a chosen id; its hooks receive `session_id` on stdin (`SessionStart` with `source`), and `codex resume <SESSION_ID>` reopens it (`codex resume` alone opens a picker filtered to the cwd; `--last` could pick another pane's session in the same directory). **(2) restore after `kill -9` ✅**: the server keeps `$XDG_STATE_HOME/monarch/<session>/state.json` (else `~/.local/state/…`; dir 0700, file 0600, written atomically by temp file + rename) with each pane's argv, live working directory (`ProcessProbe` seam; `SystemProbe` reads it through `sysinfo`, which needs no build tools; `libproc` was rejected because its `pidcwd` is unimplemented on macOS and it needs bindgen) and the focused pane. The `Persister` task rewrites it when a pane starts or exits and re-reads directories every 2 s (`PERSIST_INTERVAL`), writing only on change. `monarch server` without `--pane` (so every auto-start) restores a saved session: a shell comes back as a shell in its directory, and any other command comes back as a shell there, never re-run (it may not be safe twice), under a dim notice "monarch: restored after the server stopped; it was running: …". A vanished directory, a pane that won't start or a damaged file never stops the session from starting. Ending the session on purpose (last pane exits) deletes the file. The client now reaps the server it auto-starts (it lingered as a zombie if the server died while attached). Headless `scripts/spike6_restore_smoke.py` (in `just smoke` and CI): live cwd saved, `kill -9` → shell back in its directory under the notice, a command pane not re-run, clean exit deletes the state. **(3) agent panes ✅**: `monarch server --agent claude|codex` and `monarch attach --agent …` (a new session only; refused for a running one) start agent panes. `AgentKind` lives in core; the server's `PaneAgent` records the kind and session id with the pane. Claude Code starts as `claude --session-id <uuid>` (a fresh v4 UUID per pane) and comes back as `claude --resume <uuid>`; Codex starts bare and comes back as `codex resume <id>` once its hooks report the id (M3), until then `codex resume` (its picker, filtered to the pane's directory; never `--last`, which could take another pane's session). The restore notice names the session. The smoke test covers both with stand-in agent scripts that record their arguments. **Remaining (manual):** `monarch attach -s try --agent claude`, say something, `pkill -9 -f 'session try'`, `monarch attach -s try`: the conversation should be back.
- [x] M2 multiplexing & navigation (2026-10-25). Slices: (1) split tree in core · (2) server tabs and splits · (3) client draws several panes, click to focus · (4) keymap with presets · (5) which-key, repeatable keys, tab bar · (6) palette and go-to picker · (7) copy mode and search · (8) sidebar skeleton · (9) config hot reload
  - [x] (1) split tree: `monarch-core` `layout` module with `Rect`, `SplitAxis` (`Columns` = side by side, `Rows` = stacked), `Direction` and `SplitTree` (binary tree of panes, ratios in thousandths, serde for the state file). Operations: `split`, `remove` (the sibling takes the space), `swap`, `equalize` (even shares along each axis, whatever the nesting), `layout(area)` (one-cell dividers; both children keep at least a cell), `pane_at`, `neighbour` (most shared edge across a divider, then topmost or leftmost) and `resize` (moves the nearest matching divider, tmux semantics). A proptest checks that random split/close sequences tile the window without overlap.
  - [x] (2) server tabs and splits: core `Tab` (tree, focus, zoom; focus by direction or cycle; closing the focused pane hands focus to the pane that takes its space) and `Action` (the one list of commands for keymap, palette and protocol; `Detach`/`ToggleMouse` are client-local); `ClientMsg::Action`. `SessionState` holds tabs: every pane (hidden tabs too) is sized to its rectangle, so switching tabs never reflows; a zoomed pane fills the window while the others keep their split sizes; panes that become visible get snapshots. Split and new tab start a shell in the focused pane's current directory (`ServerContext::spawn_shell_at`, via the process probe); close hangs the program up (`PaneIo::close`) and the normal exit path removes it. Tests on fake panes plus an end-to-end split over a real socket. Not yet: keys for these (slice 4), drawing several panes (slice 3); new panes use `$SHELL`, not `[terminal].shell`; the state file still restores each pane as its own tab (layout persistence comes with the tab bar).
  - [x] (3) client draws splits: `PaneBorders` widget draws box-drawing dividers in every cell no pane covers, joined into corners, tees and crosses, bold beside the focused pane and dim elsewhere (no colours until theme tokens). A press on another pane sends `ClientMsg::Focus` and swallows the rest of that click (`SharedFocus::set_panes`/`pane_at`); everything else is routed for the focused pane as before. Still to come in the mouse item: dragging a divider to resize, wheel over an unfocused pane.
  - [x] (4) keymap: core `Key` (a character with Shift applied, an arrow, Tab, Enter or Esc; parsed from `%`, `H`, `shift+h`, `left`, `minus`, `space`…), `KeymapPreset` (`monarch`: tmux keys plus `|`/`-` splits, `hjkl` focus, `HJKL` resize, `=` equalize; `tmux`: tmux's defaults, tab `0` first; `herdr`: `v`/`-` splits, `hjkl` focus, `HJKL` swap, `q` detach) and `Keymap` (a preset plus `[keys.bind]`, e.g. `v = "split-right"`, `x = "none"` to unbind). Action names (`split-right`, `focus-left`, `tab-3`…) live once in `action/names.rs`; new `swap-*` actions move the focused pane. A bad preset, key or action becomes a warning (status line for 8 s at attach, `monarch config check`) and never costs the user their keys. The client maps the command key through the keymap: `detach` and `toggle-mouse` run in the client, everything else goes to the server as `ClientMsg::Action`. The prefix parser now reads an escape sequence after the prefix as an arrow (CSI or SS3, any modifiers) or a lone Esc, swallows other sequences (F-keys, Alt+x) instead of typing them into the pane (prefix+arrow used to type `[D`), and passes an SGR mouse report through while the prefix keeps waiting. Status-line and nesting hints name the configured detach key. `monarch keys` lists the bindings. `scripts/spike4_attach_smoke.py` now splits with `prefix %` and moves back with `prefix ←` through the real binary.
  - [x] (5) which-key, repeatable keys, tab bar: after the prefix waits `which_key_delay_ms` (300) a popup lists the keymap, grouped into one line per meaning (`hjkl ←↓↑→ focus`, `1-9 go to tab`), in as many columns as fit (`WhichKeyEntry`, `WhichKey` widget); the next key or Esc closes it. Resize and swap keys repeat without the prefix for `repeat_ms` (500; 0 turns it off) after each use: arrows, characters and kitty reports, inside the prefix parser so it stays in the input thread; anything else closes the window and reaches the pane. Focus moves deliberately do not repeat, since repeating them would swallow the first keys typed into the pane just focused. The status line is now a tab bar: `LayoutMsg` carries every tab (`TabSummary`: focused pane, its title, zoom) and the shown index; labels are `n title` (titles follow `Title` messages, cut at 20 characters; `[Z]` when zoomed), the shown tab is the one run not in reverse video, and notices replace the key hint on the right instead of the title. `attach_app` split into `draw.rs` and `which_key.rs`. Spike 4 smoke covers the popup, `prefix c` and `prefix 1`. Not yet: tab rename/close keys (herdr `T`/`X`), tab bar mouse clicks.
  - [x] (6) palette and go-to picker: `prefix :` opens the command palette (every action in `Action::catalogue()` plus one row per tab, each with its bound key) and `prefix g` (also `w`; tmux preset `w`) the go-to picker (every pane in every tab as `tab title`, `here` beside the focused one), both fuzzy-searched with `nucleo-matcher` (MPL-2.0, already allowed by `deny.toml`): words match separately, matched characters are bold, best match first and ties keep list order. One `Picker` model (pure data; `PickerItem` rows each run an `Action`, `PickerStep` says what a key did) and one `PickerView` widget (box near the top, query line with the cursor in it, scrolling list, footer hint) serve both. New actions `command-palette` and `go-to-pane` (client-local) and `FocusPane(PaneId)` (what the picker runs; not bindable; the server focuses the pane and shows its tab). `TabSummary` now lists every pane with its title (`title()` gives the focused one), so the picker can name panes in hidden tabs. While a pop-up is open, `SharedFocus::overlay` sends host input to the main loop (`ClientEvent::OverlayInput`) instead of the pane; the input thread sets it itself on a pop-up key, so text typed in the same read as `prefix :` reaches the pop-up, not the shell. `OverlayKey` decodes plain bytes, arrows, Ctrl-n/p/u/w/c/g, Tab/Shift-Tab and kitty reports, and drops mouse reports, paste markers and Alt chords. The which-key label for the palette is `palette` so three columns still fit 80 cells. Spike 4 smoke runs `split down` from the palette and jumps to tab 2 with `prefix g ^2 Enter` typed in one write; the harness now keeps reading the client's output while it waits (an unread pty blocked the client, which then sent keys to a pane that had just exited). Not yet: agent-state filters in the go-to picker (M3); keys typed in the same read after the pop-up closes are dropped; in kitty mode with release events on, the prefix parser may still swallow one later release of the key that opened a pop-up.
  - [x] Pane names: most shells and editors never set a title, so tabs read `1`, `2`. Like tmux's automatic rename, the server's `CommandWatcher` reads every pane's foreground process (`PaneIo::foreground_process`: the PTY's foreground process group via portable-pty's `process_group_leader`) and its name (`ProcessProbe::name`, `sysinfo`; a login shell's leading `-` dropped) every 500 ms (`COMMAND_INTERVAL`), without holding the session lock while it asks the OS, and broadcasts `ServerMsg::Command` only on change. `TabSummary` panes are now `PaneSummary { pane, title, command }`; a pane is named by a non-blank title, else its command (`PaneSummary::pick`), in the tab bar and the go-to picker. The host window title still uses only the pane's own title. Spike 4 smoke checks the tab reads `sh`, then `cat` while `cat` runs, then `sh` again.
  - [x] (7) copy mode and search: `prefix [` (all three presets; action `copy-mode`, client-local) starts vi-style copy mode on the focused pane. Keys: `hjkl`/arrows, `w b e`, `0 ^ $`, Home/End, `H M L`, Ctrl-u/d, Ctrl-b/f, PgUp/PgDn, `g`/`G`; `v`/Space chars, `V` lines, Ctrl-v block (same key again drops it); `y`/Enter copies and leaves; `/` searches older text, `?` newer, `n`/`N` repeat (smart case, matches drawn bold+underlined, "not found" notice); `q`/Ctrl-c leave, Esc drops the selection then leaves. `CopyMode` is pure data over a `Screen` (cursor in absolute `GridPoint` lines, `GridPoint` moved to proto) and asks the server to scroll with `ClientMsg::ScrollTo{top}` (absolute, clamped; `i64::MAX` = live) so the cursor stays in view. Text and search live in the server's engine: `Engine::text_range` (alacritty `Selection` + `selection_to_string`, so wrapped lines join and blocks work) and `Engine::search` (`RegexSearch` on the escaped query); `ClientMsg::Copy`/`Search` are answered only to the asking client (`ServerMsg::Clipboard` → OSC 52, `ServerMsg::SearchResult`). Mouse selection now copies through the same server path, so a drag that spans scrolled-off lines copies the whole range. `OverlayKey` gained PageUp/PageDown/Home/End/Enter/Backspace/Esc and generic Ctrl chords. Spike 5 smoke yanks a line with `prefix [ k0v$y` and yanks from a `/` search hit, both checked as OSC 52. Not yet: search runs under the pane lock (fine for 10k lines, revisit for huge scrollback); absolute line numbers drift once scrollback is full and old lines are dropped while copy mode is open; copy mode leaves if the focused pane changes.
  - [x] (8) sidebar skeleton: on hosts at least `[ui].sidebar_min_columns` wide (100) a `[ui].sidebar_width` (32) column sidebar lists every pane in every tab, grouped under its git checkout (`▾ project  branch`, first seen first) with panes outside any repository under `Scratch` last. A row is `tab name` plus where it works (`src` below the checkout's top, the directory's name outside one); the focused pane is marked `▸` and bold, panes in hidden tabs are dim. A click on a row focuses that pane and shows its tab; `prefix t` (`toggle-sidebar`, client-local, all presets) hides or shows it, and the panes are resized to the room left. Server: `PlaceWatcher` reads each pane's foreground directory every second (`ProcessProbe::cwd`) and asks git only when it changes, or every fifth check so a `git switch` shows (`RepoProbe` over the new `GitRunner` seam; `CliGitRunner` runs `git rev-parse --path-format=absolute --show-toplevel --git-common-dir`, then `symbolic-ref --short -q HEAD` or a short commit id, with `GIT_OPTIONAL_LOCKS=0`, on a blocking thread). The worktree's project is named after the shared git dir, so linked worktrees group under one project name with their own branch. `PanePlace` travels in `PaneSummary.place` and `ServerMsg::Place`. Client: `AttachState::sidebar_groups()` builds the model, `Sidebar`/`SidebarGroup`/`SidebarRow` draw it, `AttachState::set_left` shifts pane rectangles on the host (drawing, clicks and selection all follow), and sidebar rows join the click targets the input thread hit-tests. Spike 4 smoke attaches a 120-column host in a fresh repository, sees the project and branch, checks the shell has 88 columns, clicks a row to focus a pane in another tab, and checks `prefix t` gives the shell 120 columns. Not yet: agent state glyphs and activity lines (M3), the Inbox heading, collapsing groups, drag-to-resize, scrolling a sidebar taller than the window (rows that do not fit are cut off), long names are cut at the divider without an ellipsis, and the sidebar is redrawn with every frame.
  - [x] Drag auto-scroll: a selection drag held past the focused pane's top or bottom edge scrolls it (`DragScroll`: one step every 50 ms, one line per row past the edge up to 8) and the selection's head follows the edge row through the new text, so a single drag selects more than a screen and the release copies all of it through the server. The pointer cannot go above the host's top row, so on a pane touching it that row counts as one past the edge once the drag has left the row it started on (`MouseRouter` reports `SelectEvent::Extend { beyond }`). Dragging past the bottom scrolls back towards live output. Spike 5 smoke drags onto the top row, holds for 1.5 s and checks the OSC 52 copy holds lines that had scrolled off.
  - [x] (9) config hot reload: the client re-reads the config file when its modification time changes (`ConfigWatch`, checked once a second; creating or deleting the file counts) or on `prefix R` (`reload-config`, client-local, all presets). One conversion, `AttachOptions::new`/`configure`, sets the client from a `LoadedConfig` at startup and on every reload: keymap and preset, prefix (the input thread gets new `InputKeys` over a channel and rebuilds its prefix parser before parsing the next read), which-key delay, repeat window, mouse policy (unless `prefix m` handed the mouse to the host), window title and the sidebar (width and threshold apply at once; a sidebar hidden with `prefix t` stays hidden unless the file's `sidebar` itself changed). A file that fails to load or names a non-control prefix changes nothing and the status line says why (`config not reloaded: …`); a good one says `config reloaded`, plus any warnings such as unknown keys (now also shown at startup). Spike 4 smoke edits the config while attached: the sidebar widens to 40 columns, the prefix becomes Ctrl-a, and a bad prefix is refused with the old keys still working. The tab-bar check in that smoke now rebuilds the bottom row from all output (only changed cells are redrawn; CI saw a label split around an unchanged cell). Not yet: server-side settings (`[terminal]` shell, scrollback, env; `[agents]`) apply only to a new server; config is per client, so two clients may use different keys.
  - [x] Right-click menu: a right-click on a pane that is not tracking the mouse (or Shift+right-click on one that is) opens a small menu at the pointer: split right/down, zoom, close pane, new tab, copy mode, toggle sidebar, command palette, each with its bound key. A click on a row runs it, a click elsewhere or Esc/`q` closes it, Up/Down/`j`/`k` and Enter work too. Right-clicking another pane focuses it first, so the menu acts on the pane clicked. `ContextMenu` (pure model) and `ContextMenuView` (placed beside the pointer, kept on screen, `item_at` for clicks); the router emits `MouseAction::Menu`, the input thread switches to overlay input like a pop-up key, and the main loop splits overlay bytes into keys and mouse reports. Spike 5 smoke opens it, closes it with Esc, then clicks `split right` and checks the shell has half the columns. `attach_app.rs` hit the size cap, so its startup helpers moved to `attach_app/startup.rs`.
  - [x] Multi-client: two clients on one session share panes and input; the window follows the client that resized last (tmux `window-size latest`). Fixed: when that client detached, the window kept its size, so a smaller terminal left attached was drawn clipped. Each `ClientHandle` now remembers its last reported size and `remove_client` resizes to the most recent one still attached. Spike 4 smoke attaches an 80- and a 120-column client: typing in one shows in the other, the shell sees 88 columns (120 less the sidebar), and after the big one detaches it sees 80 again.
  - [x] Mouse badge: the status line always shows who has the mouse, `MOUSE: MONARCH │` (the default at start) or `MOUSE: TERMINAL │`, before the hint and beside notices and `PREFIX`. It reads what the host was last told (`HostModeMirror::captures_mouse`), so `prefix m`, `mouse = off` and `mouse = pane` over a pane that isn't tracking all show TERMINAL. On rows too narrow for the session name, badge and hint, the badge is dropped first. Manual check (2026-10-25, iTerm2): mouse and selection work (spike 5 confirmed); triple-click and Shift+drag crossing panes was iTerm holding the mouse, which this badge now makes visible. Spikes 1, 3 and 6 manual checks were not individually confirmed.
- [ ] M3 agent awareness. Slices: (1) agent state from Claude Code hooks · (2) `monarch setup` · (3) OSC tap signals and Codex · (4) Attention Inbox.
  - [x] (1) hooks → state: the session socket now also speaks a JSON-lines API (`Connection` reads the hello and hands `"attach"` to `AttachConnection`, `"json"` to `ApiConnection`): one `Request {id, method, params}` per line, one `Response {id, result|error}` per line (`RpcError` codes -32700 parse, -32601 method, -32602 params, -32001 no such pane; 64 KiB line cap). Its only method so far is `agent.report` (`AgentReport {pane, agent, state?, activity?, session_id?}`). A hooks-only `StateEngine` per session keeps each pane's `AgentStatus {state, activity}` (activity = first non-blank line, ≤ 200 chars) and broadcasts `ServerMsg::Agent` on change; `PaneSummary.agent` carries it in layouts. `monarch hook claude` reads the hook JSON on stdin (≤ 8 MiB), maps it with `ClaudeHook` (SessionStart → Idle; UserPromptSubmit → Working + prompt; PreToolUse → Working + `ToolActivity` like `Edit src/x.rs` / `Bash cargo test`; PermissionRequest and permission/question notifications → Blocked; idle reminders ignored; Stop → Done + `last_assistant_message`; StopFailure → Error; SessionEnd → back to a shell) and posts it to `MONARCH_SOCKET` for `MONARCH_PANE` (ids now parse from `p7`). Outside Monarch it does nothing; failures are one stderr line and exit 0, so a hook never interrupts the agent. The sidebar shows every row's state glyph (`$` shell, `●` working, `◆` blocked, `✓` done, `✗` error…; ones needing attention in bold reverse video) and the activity in place of the directory. Until `monarch setup` exists, hooks can be added by hand to `~/.claude/settings.json` (each event → `{"type":"command","command":"monarch hook claude","timeout":5}`). Not yet: Done decays to Idle when the pane is seen (with the Inbox, slice 4); the reported `session_id` is not yet recorded for restore (needed for Codex, slice 3).
- [ ] M4 projects & worktrees
- [ ] M5 persistence & restore
- [ ] M6 polish & 0.1 release
- Next up: M3 slice (2): `monarch setup` merges Monarch's hooks into `~/.claude/settings.json` (idempotent, backup, `--dry-run`, `--uninstall`, never touching unknown keys), then (3) OSC tap signals and Codex, (4) Attention Inbox. Release PR #4 stays unmerged by the user's decision (2026-10-25): nothing is published.
- Known gaps: the client starts the server with its own environment (`monarch server` inherits the attaching shell's cwd and env; fine for one user, revisit for `monarch new`); the startup probe is sent once per attach, not re-sent on FocusGained; the host title is set to the pane's (possibly empty) title on attach and restored on exit; memory is mostly scrollback (≈70 MB per flooded 200-column pane at 10k lines; use `agent_scrollback` for agent panes or compact history later); a pane flooding output costs about one core of parsing even when hidden (inherent to G13; real agents are far below that rate); the writer channel is unbounded (a huge paste into a program that never reads stdin grows memory); mouse reports pass through unre-encoded in the prototype (correct only for one full-screen pane; re-encoding is spike 5); the server must keep the last client's `HostInfo` so panes started before an attach still answer OSC 10/11 with host colours; Claude Code OSC 7501 probe behaviour (spike 2); crates publish as `monarch-mux-*` (`monarch-core` was taken on crates.io; `[lib] name` keeps `monarch_core` imports) and release-plz publishes them in dependency order; releases need repo secrets `RELEASE_PLZ_TOKEN` (PAT), `HOMEBREW_TAP_TOKEN` (PAT with write to `cooksterj/homebrew-monarch`) and `CARGO_REGISTRY_TOKEN`; installer artifact is `monarch-mux-installer.sh`; ratatui has one underline modifier, so undercurl/dotted/dashed/double underlines currently render as plain underline (fix needs a custom backend writer — a G-contract gap); macOS blocks a process exiting until its tty output is drained, so PTY test harnesses must keep reading

## 6. Risks & mitigations

| Risk | Mitigation |
|---|---|
| OSC interception without forking alacritty | Settled: `vte::Performer` is private and unknown OSCs are only debug-logged, so `OscTap` (byte-level, memchr-gated) is the production path; a second `vte::Parser` is the test oracle. Spike 2 proves parity and < 5 % overhead. |
| Claude Code emits no OSC 9 under `TERM_PROGRAM=monarch` | By design (no impersonation). Hooks are authoritative and richer; `monarch doctor` warns if hooks aren't installed; BEL fallback available. |
| Kitty-keyboard / mouse mode mirroring edge cases | Mirror on every focus change + detach; integration tests with a probe binary; `prefix m` passthrough as an escape hatch. |
| Herdr manifest schema drift | Pin the vendored manifest version; honour `min_engine_version`; fixture tests per manifest; user overrides dir. |
| Hook install edits user files | Backup, idempotent merge, `--dry-run`, `--uninstall`, never touch unknown keys. |
| Unicode width mismatch with host | Pin `unicode-width` to alacritty's version; grapheme tests. |
| Scope creep | v1 exclusions above; every milestone ends in a usable checkpoint. |

---

## 7. File tree (one primary type per file; the type each file owns is in parentheses)

```
Cargo.toml  rust-toolchain.toml  dist-workspace.toml  deny.toml  CLAUDE.md  README.md  CHANGELOG.md
LICENSE-MIT  LICENSE-APACHE  .github/workflows/{ci,release}.yml
docs/plan.md  docs/architecture/terminal-core.md        # this plan + the core design, copied in at M0
src/main.rs                                              # → monarch_cli::run()
manifests/*.toml  manifests/ATTRIBUTION.md               # vendored Herdr manifests (Apache-2.0)
tools/vtprobe/src/main.rs                                # fidelity probe binary
tests/fidelity/{harness.rs, keys.rs, mouse.rs, queries.rs, paste.rs, attach.rs}

crates/monarch-core/src/
  lib.rs  error.rs (CoreError)
  ids.rs (SessionId, TabId, PaneId, ClientId newtypes)
  agent_state.rs (AgentState)   agent.rs (Agent)   pane.rs (Pane)   tab.rs (Tab)
  project.rs (Project)   worktree.rs (Worktree)
  layout.rs + layout/{split_tree.rs (SplitTree), rect_solver.rs (RectSolver), preset.rs (LayoutPreset)}
  config.rs (Config) + config/{keys.rs (KeysConfig), ui.rs (UiConfig), theme.rs (ThemeConfig),
          notifications.rs (NotificationsConfig), agents.rs (AgentConfig), terminal.rs (TerminalConfig),
          projects.rs (ProjectsConfig), session.rs (SessionConfig), loader.rs (ConfigLoader)}
  keymap.rs (Keymap) + keymap/{chord.rs (Chord), action.rs (Action), preset.rs (KeymapPreset)}
  theme.rs (Theme) + theme/{tokens.rs (ThemeTokens), builtin.rs (BuiltinThemes)}

crates/monarch-proto/src/
  lib.rs  version.rs (Hello, HelloAck)
  api.rs + api/{envelope.rs (Request, Response, RpcError), event.rs (ApiEvent),
                 session.rs, tab.rs, pane.rs, agent.rs, worktree.rs, layout.rs, events.rs}   # params/results per namespace
  attach.rs + attach/{server_msg.rs (ServerMsg), client_msg.rs (ClientMsg), layout_msg.rs (LayoutMsg),
                       snapshot.rs (Snapshot), delta.rs (Delta, RowDelta, Run), style.rs (StyleDef),
                       host_info.rs (HostInfo), codec.rs (AttachCodec)}

crates/monarch-term/src/
  lib.rs  error.rs (TermError)
  engine.rs (trait Engine)   screen_snapshot.rs (ScreenSnapshot)   shadow_grid.rs (ShadowGrid)
  selection_state.rs (SelectionState)   scrollback_search.rs (ScrollbackSearch)
  osc.rs (Osc)   osc_tap.rs (OscTap)
  alacritty.rs + alacritty/{engine.rs (AlacrittyEngine), listener.rs (PaneListener),
                              term_config.rs (TermConfigBuilder), cell_convert.rs (CellConverter)}

crates/monarch-detect/src/
  lib.rs  error.rs (DetectError)
  manifest.rs (Manifest)   rule.rs (Rule)   gate.rs (Gate)   region.rs (Region)
  evaluator.rs (Evaluator)   verdict.rs (Verdict, Evidence)   registry.rs (ManifestRegistry)
  process_probe.rs (trait ProcessProbe) + process_probe/{macos.rs, linux.rs}

crates/monarch-server/src/
  lib.rs  error.rs (ServerError)
  server.rs (Server)   session.rs (Session)   event_bus.rs (EventBus)   cwd_tracker.rs (CwdTracker)
  pane_runtime.rs (PaneRuntime) + pane_runtime/{spawn.rs (PaneSpawner), reader_thread.rs (ReaderThread), writer_thread.rs (WriterThread)}
  frame_coalescer.rs (FrameCoalescer)
  connection.rs + connection/{attach_connection.rs (AttachConnection), api_connection.rs (ApiConnection), lanes.rs (ClientLanes)}
  api.rs + api/{dispatcher.rs (ApiDispatcher), session_handler.rs, pane_handler.rs, agent_handler.rs,
                 tab_handler.rs, worktree_handler.rs, layout_handler.rs, events_handler.rs}
  state_engine.rs (StateEngine) + state_engine/{signal.rs (Signal), hook_ingest.rs (HookIngest),
                 osc_ingest.rs (OscIngest), screen_ingest.rs (ScreenIngest), quiescence.rs (QuiescenceTimer)}
  notifier.rs (Notifier) + notifier/{channel.rs (trait NotificationChannel), rules.rs (NotificationRules),
                 toast_channel.rs, terminal_osc_channel.rs, bell_channel.rs, command_channel.rs}
  persist.rs + persist/{state_file.rs (StateFile), schema.rs (PersistedSession), restorer.rs (Restorer)}
  git.rs + git/{git_runner.rs (trait GitRunner + CliGitRunner), repo_probe.rs (RepoProbe),
                 worktree_manager.rs (WorktreeManager), diff_reader.rs (DiffReader)}
  platform.rs + platform/{macos.rs, linux.rs}

crates/monarch-tui/src/
  lib.rs  error.rs (TuiError)
  app.rs (App)   app_state.rs (AppState)   view_model.rs (ViewModel)   render.rs (Renderer)
  server_connection.rs (ServerConnection)
  input.rs + input/{tty_reader.rs (TtyReader), prefix_parser.rs (PrefixParser), mouse_translator.rs (MouseTranslator), paste_guard.rs (PasteGuard)}
  host.rs + host/{host_mode_mirror.rs (HostModeMirror), host_probe.rs (HostProbe), host_clipboard.rs (HostClipboard)}
  actions.rs + actions/{dispatcher.rs (ActionDispatcher), pane_actions.rs, tab_actions.rs, agent_actions.rs, session_actions.rs}
  widgets.rs + widgets/{status_bar.rs, tab_bar.rs, sidebar.rs, pane_view.rs, pane_border.rs, hint_line.rs, which_key.rs, toast.rs, modal.rs}
  views.rs + views/{inbox.rs, command_palette.rs, goto_picker.rs, review.rs, help.rs, copy_mode.rs,
                     worktree_wizard.rs, agent_info.rs, welcome.rs}

crates/monarch-cli/src/
  lib.rs  cli.rs (Cli)
  commands.rs + commands/{attach.rs, new.rs, ls.rs, kill.rs, server.rs, api.rs, hook.rs, setup.rs, doctor.rs, config.rs, agent.rs, pane.rs}
  hook.rs + hook/{claude_hook.rs (ClaudeHook), codex_hook.rs (CodexHook), codex_notify.rs (CodexNotify), report_mapper.rs (ReportMapper)}
  setup.rs + setup/{claude_settings.rs (ClaudeSettingsEditor), codex_config.rs (CodexConfigEditor),
                     monarch_config.rs (MonarchConfigWriter), backup.rs (BackupStore)}
  doctor.rs + doctor/{check.rs (trait Check), checks.rs, report.rs (DoctorReport)}
```

M0 creates the skeleton (crates, `lib.rs` files, CI, dist, docs); files are added milestone by milestone following this map.

---

## 8. Verification

**Automated**
- Unit: config/keymap parsing (incl. presets), split-tree layout math, state fusion transitions, manifest evaluator against per-agent fixtures (recorded screens for working/blocked/idle), proto round-trips, hook JSON → report mapping.
- Integration (`tests/fidelity/`): a host PTY runs `monarch` attached to a temp session; `vtprobe` runs inside the pane and asserts G1–G7, G11, G12 byte-for-byte (key encodings incl. kitty flags, mouse reports, bracketed paste, OSC 10/11/52 replies, DA/DSR, focus events, title). Detach/reattach and two-client attach.
- Performance (criterion + a `bench` subcommand): parse throughput, render time with 1/15/30 panes, key→PTY-write latency histogram; CI asserts no regression > 20 %.
- Install: `cargo install --path .`, `cargo publish --dry-run` per crate, `dist plan`/`dist build` artifacts, `cargo clippy -D warnings`, `cargo fmt --check`.

**Manual (release checklist)**
- Compatibility matrix (§2) incl. SSH; Claude Code: Shift+Enter, theme detection, permission prompt → Blocked + inbox excerpt + quick answer, Stop → Done summary; Codex: approval → Blocked via OSC, turn complete → Done; `vim`/`htop`/`fzf`/`less` behave identically to a bare terminal; paste 50 KB; drag-select during heavy output; kill server → restore.
