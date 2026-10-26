# M6: 20 panes through the real client

Run with `just bench-attach` (release) or
`python3 scripts/perf_attach.py target/debug/monarch` (the debug build
developers run). Unlike spike 4, which used protocol clients, this drives
the real `monarch attach` client in a PTY at 200×50, with the sidebar,
themes, animations, agent detection, git probes and persistence all on.
State, config and sockets live in a temporary directory.

## Scenarios

Every pane starts in its own tab, so the sidebar lists all 20.

- **idle**: 20 panes running `sleep`.
- **working**: 20 fake agents report Working through `monarch hook claude`,
  so all 20 sidebar dots animate.
- **busy**: a focused probe pane, 9 working agents, and 10 agents in other
  tabs printing a line every 20 ms (a fast agent's rate). One key is typed
  into the client every ~10 ms. Each is timed until it comes out of the
  probe pane (`stty raw; cat > fifo`): client → server → PTY → `cat`.
- **visible**: like busy, but the first tab is split four ways, so three
  of the printing panes are on screen beside the probe.

CPU is cumulative process time over the window. "To host" is what the
client wrote to the terminal.

## Results (2026-10-26, Apple M2 Max, 12 cores, macOS 26.6)

Release, after the fixes below:

| scenario | client CPU | server CPU | to host/s | client RSS | server RSS | key p50 | key p99 | key max |
|---|---|---|---|---|---|---|---|---|
| idle | 0.2 % | 0.5 % | 0 B | 9 MiB | 30 MiB | | | |
| working | 0.7 % | 0.6 % | 2.3 kB | 10 MiB | 30 MiB | | | |
| busy | 0.9 % | 1.0 % | 2.2 kB | 10 MiB | 90 MiB | 87 µs | 266 µs | 508 µs |
| visible | 3.4 % | 1.9 % | 102 kB | 11 MiB | 78 MiB | 82 µs | 253 µs | 405 µs |

Debug, after the fixes below:

| scenario | client CPU | server CPU | to host/s | key p99 | key max |
|---|---|---|---|---|---|
| idle | 0.2 % | 0.6 % | 0 B | | |
| working | 4.0 % | 0.6 % | 2.4 kB | | |
| busy | 3.2 % | 1.5 % | 2.3 kB | 274 µs | 784 µs |
| visible | 13.1 % | 7.7 % | 93 kB | 260 µs | 467 µs |

## What changed

1. **Rendering and periodic scans run off the async threads.** The frame
   coalescer rendered (capture, diff and encode every visible pane under
   the session lock) directly on a tokio worker. The screen watcher
   (detection manifests), command watcher and persister did the same.
   A keystroke whose socket wakeup was queued on that worker waited for
   the whole render. Tokio's LIFO slot can't be stolen by idle workers.
   They now run in `spawn_blocking`. With a debug server and three busy
   panes on screen, key p99 went from **1.2–1.8 ms to 0.26 ms**. Mixing
   builds located the cause: a debug client with a release server stayed
   at 0.32 ms.
2. **Keys don't take the session lock.** `InputRouter` keeps each pane's
   input channel behind its own small lock, so `ServerContext::send_input`
   queues the bytes without waiting for a frame. Noting that the user
   answered a blocked agent moved to the event loop (`PaneEvent::Replied`).
3. **A client leaving now wakes the renderer.** Removing a client
   resizes the window to fit the clients left. That marks their panes
   dirty, but it didn't wake the frame coalescer. The PTY reader only
   wakes it when a pane goes from clean to dirty, so the remaining client
   got nothing more until something else forced a render. A wake on every
   keystroke had hidden this. The spike 4 smoke caught it once input
   stopped taking that path.
4. **Debug builds optimise dependencies** (`[profile.dev.package."*"]
   opt-level = 2`). In debug, alacritty, ratatui and postcard were
   unoptimised. Client CPU dropped from 8 % to 4 % (working) and from 22 %
   to 13 % (visible). Monarch's own crates stay at opt-level 0, so they
   are still debuggable. Dependencies compile once.

Before the fixes, release visible was p99 297–313 µs with a max of
0.98–1.26 ms. Debug visible was p99 1.5–1.8 ms with a max of 1.8–2.6 ms.
The 2 ms budget was nearly gone in the build developers actually run.

The spike 4 stress bench (`just bench-spike4`: 10 × `yes` flooding,
churn, two clients) moved from p99 311 / 427 / 467 µs (max 1.0 / 4.9 /
1.0 ms) to **240 / 265 / 244 µs (max 0.36 / 0.36 / 0.42 ms)**.

## Findings

- **Idle is free.** With 20 idle panes, neither process uses more than
  0.6 % CPU, and nothing is written to the terminal.
- **Animations are cheap but not free.** Twenty working dots cost the
  client 0.7 % CPU (release) and about 2.3 kB/s to the terminal. Each
  animation tick redraws the frame. Ratatui's diff writes only the changed
  dots, but composing the 200×50 view isn't skipped.
- **Visible output costs about 100 kB/s per three busy panes.** A scrolling
  pane changes every row, and cell diffs can't express a scroll the way
  terminal scroll regions can. That's fine locally and on any real link.
  It's the place to look if SSH bandwidth ever matters.
- **Memory is scrollback.** It's unchanged from spike 4: chatty panes grow
  toward their 10k-line history.

## Not covered (manual)

- A second client over `ssh localhost`.
- Twenty real `claude` sessions (needs a logged-in Claude Code).
