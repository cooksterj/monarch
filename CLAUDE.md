# Monarch

An attention-first terminal multiplexer for AI coding agents, in Rust. Competes with Herdr on UX
under one hard constraint: **UX is paramount, but the terminal experience must never be degraded.**

## Start of every session

1. Read `docs/plan.md`, especially the **Progress** section at the end — it says what's done and
   what's next.
2. Run `just check` (or `cargo test --workspace`) to confirm the baseline is green.
3. Continue from "Next up". Update Progress **in the same commit** as the work it describes.

Architecture details for the terminal core: `docs/architecture/terminal-core.md`.

## Locked decisions (don't relitigate)

- Prefix-only keys, default `Ctrl-b`, fully rebindable. Presets: monarch, tmux, herdr.
- Terminal engine: `alacritty_terminal` 0.26 behind an `Engine` trait. No Zig in the default build.
- Server owns PTYs + emulators; client gets cell-grid deltas (postcard, `u32`-length frames; bincode is unmaintained — RUSTSEC-2025-0141).
- Panes get `TERM=xterm-256color`, `TERM_PROGRAM=monarch`. Never impersonate the host terminal.
- Agent state: hooks are authoritative; OSC tap, Herdr-compatible screen manifests, quiescence follow.
- Package `monarch-mux`, binary `monarch`, repo `cooksterj/monarch`, MIT OR Apache-2.0.
- macOS + Linux only for v1.

## Code rules (enforced by review and `just check`)

**Files.** One primary `struct`/`enum`/`trait` per file, named after it in snake_case. Small private
helpers may sit beside their owner; anything `pub` gets its own file. Rust-2018 modules: `foo.rs`
declares children in `foo/`; no `mod.rs`, no `utils.rs`. `lib.rs` contains only `mod`, `pub use` and
crate docs. Every file starts with a `//!` line stating its single responsibility.

**Size.** ≤ 200 lines target, **300 hard cap** of non-test code per file (`just lint-size`).
Functions ≤ 40 lines, ≤ 5 parameters (use a params struct). Split a growing type by responsibility
across files (`impl` blocks can live in several files), never by adding a second type to a file.

**DRY.** One source of truth per concept (ids, `AgentState`, `Chord`, theme tokens, wire types live
once in `core`/`proto`). Use `From`/`TryFrom` instead of hand conversions. Boilerplate repeated
across types goes in one `macro_rules!` with tests (see `define_id!` in `monarch-core/src/ids.rs`).
No literal colours in widgets — use theme tokens. Defaults are declared once in `Default` impls.

**Inheritance, the Rust way.** Shared behaviour → traits with default methods. Polymorphism →
`Box<dyn Trait>` or generics. Shared fields → composition plus an accessor trait. Closed families →
enums with exhaustive `match`. Layered contracts → supertraits. Never use `Deref` to fake is-a.

**Design.** State is pure data, separate from runtime (PTYs, sockets). Render is pure:
`compute_view(&AppState) -> ViewModel`, then `render(&ViewModel, &mut Frame)`. Detection reads a
screen snapshot, never the parser. Seams are traits (`Engine`, `NotificationChannel`, `GitRunner`,
`ProcessProbe`, `Clock`) so logic is testable without a PTY. Newtype ids, enums over flags,
`#[non_exhaustive]` on wire enums.

**Errors.** `thiserror` per library crate; `anyhow` only in `monarch-cli` and `src/main.rs`. No
`unwrap()` outside tests; `expect("invariant: …")` only for true invariants.

**Performance.** Anything on the per-byte × panes × clients path is multiplicative: no allocation
per byte, terminal locks held only to copy damaged rows, hidden panes parse but never render. Std
threads for blocking PTY IO; tokio only for sockets and timers; bounded channels. CPU work and
blocking calls (rendering, detection, git, files, OS probes) go in `spawn_blocking`, never on a
tokio worker: tasks queued behind a busy worker, keystrokes among them, wait for it.

**Platform.** OS code only in `platform/{macos,linux}.rs` behind traits. `unsafe_code` is forbidden
workspace-wide (`[workspace.lints]`); a platform module that needs it must say why.

## Tests (required)

Every file defining a `pub` type has a `#[cfg(test)] mod tests` covering its public behaviour,
including at least one failure or edge case. Use fakes built on the seams, not real PTYs — only
`tests/fidelity/` spawns PTYs. `proptest` for byte/char parsers, `insta` snapshots for widgets and
views, fixture tests for each detection manifest, `criterion` for hot paths.

## Commands

- `just check` — fmt, clippy `-D warnings`, file-size lint, tests. Run before every commit.
- `just test`, `just clippy`, `just lint-size`, `just deny`, `just install`.
- `cargo run -- <args>` to try the binary.

## Workflow: trunk-based (see CONTRIBUTING.md)

- Never commit to `main` directly; it is protected. Branch from fresh `main` (`feat/…`, `fix/…`),
  keep the slice small, run `just check`, push, and open a PR whose **title is a conventional
  commit** (`feat(term): …`) — it becomes the squash commit and the changelog entry.
- `gh pr create --fill` then `gh pr merge --auto --squash`; CI must pass before it merges.
- Releases happen only by merging release-plz's `chore: release vX.Y.Z` PR. Don't bump versions
  or edit `CHANGELOG.md` by hand in feature PRs.
- Never end a session with unpushed work: push the branch (draft PR if red) and note it in
  Progress in `docs/plan.md`.
- Crates publish as `monarch-mux-*`; `[lib] name` keeps imports as `monarch_core` etc. Use
  `cargo test -p monarch-mux-term`, not `-p monarch-term`.
