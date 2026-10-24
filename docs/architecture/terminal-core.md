# Monarch — terminal core, input path, server⇄client protocol (M1 design)

APIs checked against alacritty_terminal 0.26.0 / vte 0.15.0 / portable-pty 0.9.0 / crossterm 0.29 / ratatui 0.30 sources or docs.rs. **UNVERIFIED** marks what I could not confirm.

## 1. Feeding bytes and intercepting OSCs

**Construct.** `Term::new(term::Config, &TermSize, listener)`; `TermSize` is `alacritty_terminal::term::test::TermSize::new(columns, screen_lines)` (public, not `cfg(test)`, implements `grid::Dimensions`). `Config { scrolling_history: 10_000, kitty_keyboard: true, osc52: Osc52::OnlyCopy, .. }` — `kitty_keyboard` **must** be `true`; every keyboard-mode handler early-returns otherwise. Listener: `impl EventListener for PaneListener { fn send_event(&self, e: Event) }` takes `&self`, so it holds an `mpsc::UnboundedSender<(PaneId, Event)>`. `Event`'s closures are `Arc<dyn Fn(..) -> String + Send + Sync>`, so `Term<PaneListener>: Send` and lives in `Arc<alacritty_terminal::sync::FairMutex<Term<_>>>` (`lock`, `lock_unfair`, `try_lock_unfair`, `lease`).

**Feed.** The reader thread owns a `vte::ansi::Processor<StdSyncHandler>` (vte built with `std,ansi`) and calls `processor.advance(&mut *term, &buf[..n])`. DEC 2026 is handled inside vte: after `\e[?2026h` bytes are buffered (2 MiB cap, 150 ms timeout) until `\e[?2026l` **or the caller calls `processor.stop_sync(&mut *term)`**. After each `advance`, read `processor.sync_timeout().sync_timeout() -> Option<Instant>` and use it as the `poll(2)` timeout on the master fd; on expiry call `stop_sync`. Otherwise an app that dies mid-frame leaves the pane frozen.

**Read.** `term.renderable_content()` → `RenderableContent { display_iter: GridIterator<Cell>, selection: Option<SelectionRange>, cursor: RenderableCursor { shape, point }, display_offset, colors: &Colors, mode: TermMode }`; cells are `Indexed { point, cell: &Cell { c, fg, bg, flags: Flags(u16), extra } }` with `zerowidth()`, `underline_color()`, `hyperlink()`. Dirty lines: `term.damage() -> TermDamage::{Full, Partial(iter of LineDamageBounds { line, left, right })}`, then `term.reset_damage()` once per frame. Scrollback: `grid().history_size()`, `grid().display_offset()`, `term.scroll_display(Scroll::{Delta(i32), PageUp, PageDown, Top, Bottom})`. Resize with reflow: `term.resize(TermSize::new(c, l))` **before** `master.resize(PtySize)`. Selection: `term.selection = Some(Selection::new(SelectionType::{Simple,Block,Semantic,Lines}, point, Side))`, `.update(..)`, `term.selection_to_string()`; mouse cells via `term::viewport_to_point(display_offset, Point<usize>)`.

**OSC 9/99/777 (+7, 133, 7501).** Verified: `vte::ansi::Performer` is private, `Handler` has no unknown-OSC/DCS/APC hook, and unmatched OSCs hit `_ => unhandled(params)` which only `debug!`-logs. Titles (OSC 0/2) and OSC 52 *do* arrive as `Event::Title/ResetTitle/ClipboardStore`. So a pre-parser tap is required. **Pick `OscTap`**: a four-state scanner (Ground / Esc / Osc(buf) / OscEsc) run on each chunk before `advance`; in Ground it `memchr(0x1b)`s and skips the chunk, so text costs ~0; emits `Osc { num, payload }` for an allowlist, 4 KiB cap. Rejected: a second `vte::Parser` + `Perform` (exact state machine, but ~1.5–2× parse CPU on the bytes×panes path); keep it as the test oracle.

## 2. Terminal fidelity contract

**Input path.** The client reads `/dev/tty` raw on its own std thread and forwards bytes verbatim to the focused pane as `Input{pane, bytes}` written straight to the socket — never through the render loop, never `tokio::io::stdin`. crossterm's event parser is **off** this path: it normalizes, drops unknown sequences, has no raw-bytes API, and cannot reproduce kitty `REPORT_ALL_KEYS_AS_ESC`/associated text. crossterm is used only for commands (raw mode, alt screen, `Begin/EndSynchronizedUpdate`, `SetCursorStyle`, `SetTitle`).

**Chords without ESC ambiguity.** Default prefix is a single C0 byte (Ctrl+Space `0x00`, configurable), which needs no look-ahead — no timer on the hot path. When the host is in kitty mode (mirrored from the pane) the prefix is `CSI 32;5u` and bare Esc is `CSI 27u`, so every `ESC` starts a complete sequence and is parsed to its final byte without a timeout. Optional direct Alt chords in legacy mode are recognised only when `ESC`+key arrive in the same `read()` (terminals write Alt+key atomically); a lone `ESC` is forwarded instantly. Paste bodies are never scanned.

**Kitty-encoded prefix.** Once a pane's kitty flags are mirrored, the host reports `Ctrl-b` as `CSI 98;5u` (and, with flag 8, the command key as `CSI 113u`). `PrefixParser` recognises both encodings via `KittyKey` and swallows the releases (flag 2) of keys it consumed, so the pane never sees a release without its press.

**Mode mirroring (focused pane `TermMode` → host).** Implemented by `HostModeMirror` (writes only diffs, resets before sets because xterm mouse modes are exclusive); host-only input is removed per pane by `HostInputFilter`.

| Pane state | Host action |
|---|---|
| `KITTY_KEYBOARD_PROTOCOL` bits | Startup probe `CSI ? u`; if supported, `CSI > 0 u` once. On focus/mode change `CSI = <bits> ; 1 u` (absolute set). Exit `CSI < u`. Never push/pop per pane — stack drift is the classic bug. Non-kitty host: panes get legacy bytes, as in a bare terminal. |
| `APP_CURSOR` (DECCKM), `APP_KEYPAD` (DECKPAM) | Mirrored verbatim: the host encodes arrows/keypad, so it must be in the pane's mode for keys to be byte-exact. |
| `BRACKETED_PASTE` | Host paste mode always on; strip `\e[200~…\e[201~` unless the pane set it; bytes otherwise untouched. |
| `FOCUS_IN_OUT` | Host focus reporting always on; forward `CSI I/O` only if set; synthesize `O`/`I` on pane switch. |
| mouse modes, `SGR_MOUSE`/`UTF8_MOUSE` | Host sends SGR; client subtracts the pane origin and re-encodes in the pane's encoding (SGR/UTF-8/X10). Coordinates make byte-exactness impossible; re-encoding is deterministic. |
| `ALTERNATE_SCROLL` | Host 1007 = pane `ALTERNATE_SCROLL && ALT_SCREEN` (the host itself is always on its alternate screen, so mirroring the flag alone would turn the wheel into arrows in a shell). |
| `ALTERNATE_SCROLL`+`ALT_SCREEN`, no mouse mode | Wheel → 3× `CSI A/B` (`SS3` if `APP_CURSOR`); otherwise server `scroll_display`. |
| `cursor_style()`, `SHOW_CURSOR` | DECSCUSR + blink; `Frame::set_cursor_position` or hidden. |
| `Event::Title`, `Bell` | OSC 2 on host (configurable format); BEL if pane visible, plus an attention flag. |

**Proxied requests.** At startup and on FocusGained the client probes OSC 10/11/12, `CSI 16 t`, DECRQM `CSI ? 2026 $ p`, `CSI ? u`, terminated by DA1, and sends `HostInfo{fg,bg,cursor,cell_w,cell_h,sync,kitty}`. Server answers `Event::ColorRequest(idx, fmt)` immediately from `term.colors()[idx]` if the pane overrode it, else HostInfo (256/257/258 = fg/bg/cursor) — Claude Code's theme detection works with no round trip. `TextAreaSizeRequest(fmt)` → `fmt(WindowSize{..})` from HostInfo. `PtyWrite` (DA, DSR, kitty reports) goes through the pane's writer channel in order. `ClipboardStore` → clients write `OSC 52;c;<b64>` to the host; `ClipboardLoad` stays off (`OnlyCopy`). Every ratatui draw is wrapped in synchronized-update when DECRQM reported support.

**Host-native selection.** ratatui writes only changed cells and Monarch never redraws idle panes (no ticking clock), so host selections survive. `mouse=pane` captures only while the focused pane requests mouse (native drag works elsewhere); `mouse=auto` (default) captures always, with Shift+drag passthrough and `prefix m` to toggle; `prefix z` zoom gives a single pane for clean native selection; `prefix [` copy mode uses server-side `Selection` → OSC 52.

**Pane environment.** `TERM=xterm-256color` (alacritty_terminal is xterm-class; `alacritty`/`tmux-256color` terminfo is often missing and the latter trips tmux-specific code), `COLORTERM=truecolor`, `TERM_PROGRAM=monarch`, `TERM_PROGRAM_VERSION`, `MONARCH=<socket>`, `MONARCH_PANE=<id>`; unset `TMUX`, `TMUX_PANE`, `ZELLIJ*`, `STY`. Consequence (per Claude Code docs): it sends OSC 9 only when it detects Ghostty/Kitty/iTerm2, so we will **not** get OSC 9 from Claude Code. Don't impersonate a terminal (OSC 1337 images etc. would follow). Instead `monarch init` installs Claude Code `Notification`/`Stop` hooks calling `monarch notify` (authoritative); the OSC tap, BEL (`preferredNotifChannel: terminal_bell`) and screen heuristics are fallbacks. How Claude Code probes for its OSC 7501 status protocol is **UNVERIFIED**; the tap captures 7501 regardless.

## 3. Server ⇄ client protocol

**Socket.** One server per session at `$XDG_RUNTIME_DIR/monarch/<session>.sock`, else `$TMPDIR/monarch-<uid>/`, else `/tmp/monarch-<uid>/` (0700; stale socket unlinked on failed connect). First line is JSON: `{"monarch":1,"proto":"json"|"attach","version":"0.1.0","attach":[1,1]}` → `{"ok":true,"server":"0.1.0","attach":1}`. `json` continues as JSON-RPC 2.0 lines; `attach` switches to binary.

**(a) JSON API**: `session.info`, `pane.list`, `pane.create{cmd,cwd,env,kind,split}`, `pane.kill`, `pane.send_keys{bytes|text}`, `pane.read{lines,history}`, `pane.focus`, `pane.notify{state,message}`, `layout.get/set`, `agent.state`, `subscribe{events}` → `{"method":"event","params":{"type":"pane.state|pane.title|pane.notification|pane.exit|pane.bell",…}}`, `server.version`, `server.shutdown`.

**(b) Attach — decision: server-side emulation, cell-grid deltas** (tmux/zellij model), not raw PTY bytes. The server must emulate anyway (agent-state, restore, `pane.read`), so raw forwarding means N+1 emulators and duplicated scrollback; raw attach needs a Term→escape-sequence re-serializer plus mode replay (a known bug farm); Claude Code/Codex redraw far more bytes than their net cell change, and deltas coalesce at frame rate while bytes cannot; scrollback/search/selection live once; the client is trivial. Cost: output (not input) latency gains ≤1 frame.

Codec: `postcard` (serde, varint) in frames with a big-endian `u32` length prefix and a 16 MiB limit (`FrameCodec`, usable from tokio and from blocking threads). bincode 2 was the original choice but is unmaintained since 2025-12 (RUSTSEC-2025-0141); postcard was the named fallback. Rows travel as style runs (`Run`: style id, width, cell count, text; trailing blanks dropped, wide-char spacers implied, cells with combining marks as single-cell clusters); styles are interned per connection (`StyleTable` → `DefineStyle`, `ResetStyles` when it starts over between messages; client `StyleCache`).

- S→C: `Layout{gen, tree, panes:[{id, rect, title, kind, state}]}`, `Snapshot{pane, gen, seq, rows, cursor, offset, mode_bits, cursor_style}`, `Delta{pane, gen, seq, rows:[RowDelta{line, runs:[Run{style:u16, text, cells:u16}]}], cursor, offset, mode_bits}`, `DefineStyle{id, fg, bg, ul, flags}` (per-connection 4096-entry cache), `HostWrite{bytes}` (OSC 52, BEL), `Event{json}`.
- C→S: `Input{pane, bytes}`, `Paste{pane, bytes}`, `Resize{cols, rows}`, `HostInfo{..}`, `Focus{pane}`, `Scroll{pane, delta}`, `Mouse{pane, x, y, kind}`, `Resync{pane}`, `Command{json}`.

**Backpressure.** Per client: an input lane (bounded 256; blocks only that socket reader, never a PTY) and a render lane (bounded, ~64 frames/4 MiB). On overflow the client's queued deltas are dropped and it is marked `resync`; the next tick sends `Snapshot`s of its visible panes. Per-pane monotonic `seq`; a gap triggers `Resync{pane}`.

**Resize.** Client sends `Resize`; server sizes panes (latest client wins, tmux `window-size latest`), resizes Term then PTY, bumps `gen`, sends `Layout` + `Snapshot`s. Client discards stale-`gen` deltas and repaints borders at once.

## 4. Performance architecture

- **Two std threads per pane.** Reader: `poll(2)` on `master.as_raw_fd()` (timeout = sync deadline) → `read` ≤64 KiB → `OscTap::scan` → `term.lock_unfair()` → `advance` → `dirty` + `Notify`. Writer: blocking `write` from one bounded channel carrying user input *and* `PtyWrite` replies, preserving order. portable-pty maps EIO to `Ok(0)` → child exit.
- **Tokio multi-thread runtime** for sockets, API, coalescer, agent-state; it never does PTY I/O.
- **Coalescer** (one task per session): on `Notify`, render now if idle ≥ budget, else at `last + budget` (16 ms default). For each *visible* dirty pane: `term.lock()` (fair, so a flooding reader can't starve it), `damage()` → copy only damaged rows into the shadow grid, `reset_damage()`, unlock; diff shadow → `Delta`. Lock time scales with damaged rows, not output volume.
- **Hidden panes**: parsed, damage accumulates, skipped by the coalescer; `Snapshot` on reveal. The agent-state detector runs ≤4 Hz on a text copy of the last N lines, never on the parser.
- **Typing latency**: tty read → chord check → socket → decode → `pty_tx.send` → `write(2)` ≈ 0.2–0.5 ms local; p99 target <2 ms (spike 4). Echo adds parse + ≤1 frame.
- **Pitfalls**: events fire synchronously inside `advance` under the lock — the listener may only enqueue; `damage()` is viewport-relative and over-reports unless reset each frame; 10k lines × wide panes is tens of MB per pane (default 5k for agent panes); unknown OSCs log at `debug` under target `vte` — cap it; `set_options` clears kitty stacks; build with `default-features = false` (serde unused).

## 5. Restore after server restart

Persist (`~/.local/state/monarch/<session>.json`; macOS `~/Library/Application Support/monarch/state/`; atomic rename; debounced 500 ms on layout/title/cwd change, every 30 s, on shutdown): layout tree (splits, ratios, zoom, focus); per pane: id, user title, `kind ∈ {shell, claude, codex, custom}`, argv + env overrides, start and live cwd (`master.process_group_leader()` → macOS `libproc` `PROC_PIDVNODEPATHINFO`, Linux `/proc/<pid>/cwd`, or OSC 7 from the tap), agent session id, last agent state, optional last 2000 text lines shown greyed until the restored process writes. Claude: Monarch generates a UUID and launches `claude --session-id <uuid>` (verified; must be a valid UUID), restores with `claude --resume <uuid>` (`--fork-session` optional). Codex: `codex resume` exists; resume-by-id / `--last` and where the rollout id lives are **UNVERIFIED** (spike 6). Not restorable: processes, PTY/termios state, alt-screen contents, kitty stacks, selection, unsent input.

## 6. M1 spikes

1. **Headless engine**: single-process prototype (portable-pty → `OscTap` → `Processor` → `Term` → ratatui) running `vim`, `htop`, `less`, `claude`, `codex`. Pass: parity with a bare terminal on resize/reflow, scrollback, wide/zero-width chars; sync-timeout path exercised by killing an app mid-frame.
2. **OSC tap**: replay recorded Claude Code/Codex transcripts; `OscTap` output equals a `vte::Parser` oracle; parse overhead of `cat` on 200 MB <5 %; OSC 11 answered and Claude Code picks the matching theme.
3. **Input fidelity matrix**: pane runs `cat -v`; scripted keys (Esc alone, Alt+x, Ctrl+Space, Shift+Enter, arrows under `APP_CURSOR`, F-keys, 1 MB bracketed paste, kitty flags 1 and 1|8) from kitty, ghostty, iTerm2, Alacritty, diffed against a bare terminal. Pass: byte-identical except intended chords; Shift+Enter newlines in `claude`; no Esc delay in `vim`.
4. **Latency & load**: 20 panes (10 × `yes`, 10 × idle `claude`), two clients, one over `ssh localhost`. Measure key→`write(2)` p99 (<2 ms), bytes/s per client, CPU; hidden panes ≈0 render cost; a stalled client resyncs without affecting others.
5. **Mouse & selection**: wheel in `less`/`vim`, clicks in `htop`, native Shift+drag with capture on, `mouse=pane`, zoom selection, OSC 52 reaching the host.
6. **Detach/restore**: kill client → reattach is pixel-identical via `Snapshot`; `kill -9` server → layout, cwd and `claude --resume <uuid>` restored; determine the Codex resume mechanism.
