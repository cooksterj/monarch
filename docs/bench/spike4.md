# Spike 4: latency and load

Run with `just bench-spike4` (release build). The bench is
`crates/monarch-tui/examples/spike4_bench/`. Flags: `--samples N`,
`--yes N`, `--idle N`, `--agent CMD`. Use `--agent claude` to use real idle
agents instead of the stand-in.

## Method

- `monarch server` runs as its own process with these panes:
  1. a **probe** pane: `stty raw -echo; exec cat > FIFO`;
  2. a **churn** pane: `seq` output, where every line differs;
  3. 10 × `yes`;
  4. 10 idle agent stand-ins: alternate screen, a prompt, then `sleep`.
- Two protocol clients attach at 200×50. Client A sends one key every
  5 ms to the probe pane and times it until the byte comes out of the FIFO.
  That round trip (socket → server → PTY `write(2)` → kernel → `cat` → FIFO)
  is an **upper bound** on key→`write(2)` latency.
- Server CPU comes from `ps` (macOS) or `/proc` (Linux) over each window.
  Bytes per second are what each client's socket received.
- Stall: client B stops reading while the churn pane renders. Client A keeps
  typing. B passes if it receives fresh snapshots after it resumes, which
  shows its lane overflowed and it was resynced.

`yes` is a poor render load: it repeats one line, so the shadow grid sees no
changed rows and cell deltas cost almost nothing (171 B/s with a `yes` pane
focused). That's why the churn pane exists.

## Results (2026-10-23, Apple M2 Max, 12 cores, macOS 26.6)

| scenario | p50 | p99 | max | server CPU | A recv/s | B recv/s |
|---|---|---|---|---|---|---|
| probe focused, `yes` hidden | 69 µs | 198 µs | 257 µs | 444 % | 0 B | 0 B |
| churn focused, 2 clients render | 70 µs | 272 µs | 594 µs | 445 % | 36.1 kB | 36.1 kB |
| client B stalled, A typing | 69 µs | 349 µs | 13.2 ms | 428 % | 36.7 kB | 0 B |

Server RSS was 855 MiB. The stalled client resynced. **PASS**: p99 is well
under the 2 ms budget in every scenario.

### Baselines

| panes besides probe + churn | p99 | server CPU | RSS |
|---|---|---|---|
| 10 idle, no `yes` | 104–218 µs | 75 % | 92 MiB |
| 1 `yes`, no idle | 66–200 µs | 172 % | 161 MiB |
| 10 `yes` + 10 idle | 198–349 µs | ~440 % | 855 MiB |

## Findings

- **Key latency is not affected by rendering or by the session mutex.**
  Input shares the session lock with `render_frame`, but p99 stays under
  0.4 ms even with two clients rendering a full-speed pane and ten flooding
  panes in the background.
- **Hidden panes cost nothing to render:** 0 B/s to clients while ten `yes`
  panes run hidden. They are still parsed (G13). A pane producing output as
  fast as it can costs about one core of parsing whether it's visible or
  not; the churn pane alone takes 75 %. With ten `yes` panes, the 20 busy
  threads (programs plus parsers) are more than the 12 cores, so CPU tops
  out near 440 % and latency gets an occasional scheduling outlier (one
  13 ms max out of 1000 samples). Real agents produce output far below this
  rate.
- **Memory is mostly scrollback.** A flooded pane holds its full 10k lines
  at 200 columns, about 70 MB with alacritty's cell size. Possible fixes,
  none needed for M1: use `[terminal].agent_scrollback` (5k) for agent
  panes, a smaller default, or compact storage for history rows.
- **Stalled clients are isolated:** A's latency and receive rate are
  unchanged while B is stalled, and B gets snapshots once it reads again.

## Not covered here (manual)

- A second client over `ssh localhost` (`ssh -t localhost monarch attach -s
  bench` while the bench runs). The protocol path is the same; only network
  RTT is added.
- Ten real idle `claude` panes (`--agent claude`) instead of the stand-in.
  This needs a logged-in Claude Code.
