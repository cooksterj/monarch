# Roadmap

Monarch is a terminal multiplexer for AI coding agents that shows which agent needs you, under one
hard constraint: **UX is paramount, but the terminal experience must never be degraded.** This page
says what is done, what is open and what comes next. Why things are the way they are is in
[decisions](decisions.md); how they work is in the [architecture overview](architecture/overview.md).

Keep this page short. A PR updates it when it finishes a milestone item, opens or closes a gap, or
changes what is next. The details of finished work belong in the PR and the changelog, not here.

## Status

Pre-1.0, released through release-plz and cargo-dist: tarballs, a shell installer and a Homebrew
tap. Latest release **v0.1.7** (2026-11-20 UTC); see [`CHANGELOG.md`](../CHANGELOG.md). crates.io
publishing stays off (`git_only`) until the user decides otherwise.

## Milestones

- [x] **M0 Scaffold and installability**: workspace, config, CI, cargo-dist, licences.
- [x] **M1 Terminal core**: the automated parts of all six spikes pass in CI (`just smoke`).
  Manual checks still open are listed under [Open manual checks](#open-manual-checks).
- [x] **M2 Multiplexing and navigation**: tabs, splits, keymap and presets, key popup, palette,
  go-to picker, copy mode and search, sidebar, config hot reload, mouse, multi-client.
- [x] **M3 Agent awareness**: hooks, `monarch setup`, OSC and screen-manifest detection, attention
  inbox with quick answers, `monarch agent explain/wait/read`, the Monarch skill.
- [x] **M4 Projects and worktrees**: review pane with notes to the agent, `monarch new` and
  `prefix N`, project picker, checkout badges.
- [x] **M5 Persistence and restore**: layout restore, agent resume, `monarch ls/kill`, session
  switching.
- [x] **M6 Polish and release**: themes, help, doctor, welcome, 20-pane performance pass, docs,
  v0.1.0 release.
- [x] **Sub-agents** (slices 1–2): Claude Code and Codex sub-agent rows in the sidebar, named by
  their task, blocking their parent when they ask.

## Next up

1. **File viewer** (`prefix f`), done in slice 1: search the checkout, read a file highlighted by
   tree-sitter, `V` select, `a` ask with the exact `path:lines`. Next: highlighting in the
   review's diffs (same crate), `/` search in the viewer, horizontal scroll for long lines.
2. **Ask from a selection** (`prefix e`; `a` in copy mode and review): select code, write a prompt,
   pick an agent. The prompt carries the selection's `path:lines`, found with `git grep` in the
   pane's checkout, else "from pane N". Enter sends at once to an idle agent and queues for a busy
   or blocked one until it is idle. With no agent in the checkout, it starts one with the prompt
   (a split here, or a new worktree), or several at once. Slices: ~~existing agents~~ (done:
   sends only to an agent at its prompt, refuses the rest); new agents (`NewWorktree.prompt`, a
   `Hello::WIRE` bump); source lookup; the send-when-idle queue; a right-click "ask" (opening the
   menu clears the selection today).
3. **Sub-agent slice 3**: a manifest count capture for agents without hooks (Grok's "Waiting on
   subagent…", Antigravity's "· N tasks").
4. **Mouse reports in multi-pane layouts**: re-encode coordinates for every layout, not only one
   full-screen pane.
5. **Remnant text** re-reported on 2026-11-12, unreproduced: on the next sighting, check whether it
   survives detach and reattach.

Then the differentiators, in order (why: [decisions](decisions.md#product)):

6. **Land from review**: `L` in `prefix v` commits, rebases onto the base branch, runs a configured
   check command, opens a PR with `gh`, then removes the worktree and its branch.
7. **Overlap warning**: flag on the sidebar rows, and before landing, when two worktrees' diffs
   touch the same files or hunks.
8. **Best-of-N**: `monarch race "<prompt>" --agents claude,codex,…` starts one worktree per agent
   with the same prompt; review compares the finished diffs side by side and lands the one you pick.
9. **"While you were away"**: a per-agent record of each turn from hooks (prompt, files edited,
   commands run, outcome), summarised on reattach and readable with `monarch agent log`.
10. **Context meter**: context-window fill (and spend, where the agent reports it) on each row.
11. **Public fidelity report**: run the input matrix against tmux, Zellij and Herdr and publish the
   results.

Considered, not committed: approval rules answered from the inbox ("always allow `cargo test`
here"), only with exact matching and an audit log.

Parked by the user: named layouts (`prefix Space`, 2026-11-08); notifications (toasts, host OSC
forwarding, bells), dropped on 2026-10-25 because the sidebar and inbox are the attention surface.

## Open manual checks

- **Real terminals** (kitty, Ghostty, iTerm2, Alacritty, WezTerm, Terminal.app, and over SSH):
  vim, htop, less, claude and codex at parity with a bare terminal (resize, reflow, scrollback, wide
  characters); Shift+Enter in `claude`; no Esc delay in vim; `cat -v` matches a bare terminal; wheel,
  clicks, drag/double/triple-click copy, `"+y` in vim, `prefix m`, Shift+drag native selection.
- **Spike 2**: replay recorded Claude Code/Codex transcripts through `OscTap`; measure the overhead
  of a 200 MB `cat`; how Claude Code probes OSC 7501.
- **Spike 4**: a second client over `ssh localhost`; ten real idle `claude` panes.
- **Restore**: a real `claude` conversation resumes after `kill -9` of the server.

## Known gaps

**Terminal**
- Skin-tone, flag and ZWJ emoji sequences take two emulator cells per code point against one host
  glyph.
- The startup probe is sent once per attach, not again on FocusGained, and the server should keep
  the last client's `HostInfo` so panes started before an attach still answer OSC 10/11.
- A pane flooding output costs about one core of parsing even when hidden (inherent to keeping
  hidden panes correct).
- Memory is mostly scrollback (≈70 MB for a flooded 200-column pane at 10k lines); the pane writer
  channel is unbounded, so a huge paste into a program that never reads grows memory.
- Selection copies soft-wrapped lines with a newline at the wrap (the wire has no wrap flag).
- Copy mode: search runs under the pane lock; line numbers drift once full scrollback drops lines.

**Server and sessions**
- The client starts the server with its own environment and working directory.
- Server-side settings (`[terminal]`, `[agents]`) apply only when a server starts; config is per
  client, so two clients may use different keys.
- Two Codex panes in one directory are told apart only after each has reported once.

**UI**
- Sidebar: no collapsing of groups or of an agent's sub-agents; no age per sub-agent.
- Review: no syntax highlighting (the file viewer has it), staged/unstaged split, word diff, horizontal scroll, or note to an
  agent in another pane of the checkout.
- Project picker: repositories with no pane open aren't listed; removing a worktree keeps its
  branch.
- `prefix A` always opens a tab (no split placement); dividers show no handle while dragging.
- The go-to filter keys (`b w i d`) act only on an empty query.

**Release**
- After each release, a `docs(readme): point the latest release at vX.Y.Z` PR updates the README's
  comparison row and `cargo install --tag` line.
- The README comparison table was last checked on 2026-11-05.
- `docs/assets/social-preview.png` is uploaded to the repo settings by hand whenever it changes.
- Releases need the repo secrets `RELEASE_PLZ_TOKEN` and `HOMEBREW_TAP_TOKEN` (and
  `CARGO_REGISTRY_TOKEN` once crates.io publishing is on).

## Out of scope for v1

Windows, managing remote machines, plugins, image protocols (kitty graphics, sixel), and Herdr
socket-API compatibility.
