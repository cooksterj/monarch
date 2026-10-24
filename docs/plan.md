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

Crates: `ratatui` 0.30, `crossterm` 0.29 (commands only — raw mode, alt screen, synchronized update, cursor style, title; input is raw `/dev/tty`), `alacritty_terminal` 0.26 with `default-features = false` (+ its `vte` 0.15 with `std,ansi`), `portable-pty` 0.9, `tokio` + `tokio-util` (codec), `bincode` 2 (attach stream), `serde`/`serde_json`/`toml`/`toml_edit`, `schemars`, `regex`, `memchr`, `nucleo` (fuzzy), `unicode-width` (pinned to alacritty's), `tracing`, `clap`/`clap_complete`, `directories`, `libproc` (macOS cwd). Git operations shell out to `git` (worktrees, status, diff) rather than linking libgit2 — fewer build deps, honours user git config.

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

**Attach stream = server-side emulation + cell-grid deltas** (tmux/zellij model; raw-byte forwarding rejected: the server must emulate anyway for state/restore/`pane.read`, raw attach needs a Term→escape re-serialiser plus mode replay, and agent TUIs emit far more bytes than net cell change). `bincode` 2 `standard()` with a 16 MiB limit inside `tokio_util` `LengthDelimitedCodec` (`postcard` is the fallback). S→C: `Layout{gen,…}`, `Snapshot{pane,gen,seq,rows,cursor,offset,mode_bits,cursor_style}`, `Delta{pane,gen,seq,rows:[RowDelta{line, runs:[Run{style_id,text,cells}]}],…}`, `DefineStyle` (per-connection 4096-entry cache), `HostWrite{bytes}` (OSC 52, BEL), `Event{json}`. C→S: `Input{pane,bytes}`, `Paste`, `Resize`, `HostInfo`, `Focus`, `Scroll`, `Mouse{pane,x,y,kind}`, `Resync`, `Command{json}`.

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
- [x] M0 scaffold & installability (2026-10-23): workspace + 7 crates, config system + `monarch config path|default|check`, welcome screen, CLAUDE.md conventions, CI (fmt/clippy/lint-size/nextest/msrv/deny), cargo-dist release (shell + Homebrew `cooksterj/homebrew-monarch`, formula `monarch`), licenses
- [x] Repo & delivery (2026-10-23): public `cooksterj/monarch`, trunk-based (protected `main`, squash-only, conventional PR titles), release-plz release PRs → `vX.Y.Z` tag → cargo-dist binaries/installer/Homebrew (crates.io publishing deliberately off until mature: `git_only = true`, versions from tags); Dependabot; MSRV 1.88
- [ ] M1 spikes: 1 engine · 2 OSC tap · 3 input matrix · 4 latency/load · 5 mouse/selection · 6 detach/restore
  - [~] Spike 1 engine: `AlacrittyEngine` + `Terminal` + proto `Screen` + `PaneView` + `PrefixParser` done; prototype `cargo run -p monarch-mux-tui --example single_pane [-- cmd]` passes the automated smoke (`python3 scripts/spike1_smoke.py target/debug/examples/single_pane`). **Remaining: manual parity check by the user** in a real terminal with vim, htop, less, claude, codex (resize/reflow, scrollback, wide chars), and the kill-mid-frame sync-timeout path (reader thread doesn't yet poll `sync_deadline`).
  - [~] Spike 2 OSC tap: `OscTap` matches `vte::Parser` in a property test over random chunkings. Remaining: replay recorded Claude/Codex transcripts; 200 MB `cat` overhead benchmark.
  - [~] Spike 3 input matrix: `HostProbe` (OSC 10/11/12, `CSI 16 t`, DECRQM 2026, `CSI ? u`, DA1 terminator) → proto `HostInfo` → `HostProfile`; `HostModeMirror` (DECCKM, DECKPAM, mouse 1000/1002/1003/1005/1006, 1007 gated on alt screen, kitty `CSI = bits;1u` over one push, DECSCUSR, OSC 2 title, restore on exit); `HostInputFilter` (paste markers / focus events stripped unless the pane enabled them; split markers held only from `ESC [2…`); `PrefixParser` understands kitty-encoded prefix/command keys and swallows their releases; prototype wraps frames in DEC 2026 when supported. Headless matrix `scripts/spike3_input_matrix.py` (in `just smoke` and CI) passes: keys byte-exact incl. lone Esc, Alt, F-keys, UTF-8, 1 MiB paste; modes mirrored and restored. **Remaining: manual matrix in real kitty, Ghostty, iTerm2, Alacritty, Terminal.app** (Shift+Enter in `claude`, no Esc delay in vim, `cat -v` diff vs bare terminal); re-probe on FocusGained.
- [ ] M2 multiplexing & navigation
- [ ] M3 agent awareness
- [ ] M4 projects & worktrees
- [ ] M5 persistence & restore
- [ ] M6 polish & 0.1 release
- Next up: user runs the spike-1 manual check and the spike-3 real-terminal matrix (`cargo run -p monarch-mux-tui --example single_pane -- cat -v`); meanwhile spike 4 — latency & load (key→`write(2)` p99, 20 panes, two clients)
- Known gaps: mouse reports pass through unre-encoded in the prototype (correct only for one full-screen pane; re-encoding is spike 5); the server must keep the last client's `HostInfo` so panes started before an attach still answer OSC 10/11 with host colours; Codex resume-by-id (spike 6); Claude Code OSC 7501 probe behaviour (spike 2); crates publish as `monarch-mux-*` (`monarch-core` was taken on crates.io; `[lib] name` keeps `monarch_core` imports) and release-plz publishes them in dependency order; releases need repo secrets `RELEASE_PLZ_TOKEN` (PAT), `HOMEBREW_TAP_TOKEN` (PAT with write to `cooksterj/homebrew-monarch`) and `CARGO_REGISTRY_TOKEN`; installer artifact is `monarch-mux-installer.sh`; ratatui has one underline modifier, so undercurl/dotted/dashed/double underlines currently render as plain underline (fix needs a custom backend writer — a G-contract gap); macOS blocks a process exiting until its tty output is drained, so PTY test harnesses must keep reading

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
