# Monarch

An attention-first terminal multiplexer for AI coding agents.

Monarch runs Claude Code, Codex and your shells in panes inside your existing terminal, like tmux,
and keeps track of which agents are working, which are blocked on you, and which are done, grouped
by git repository and worktree. Sessions keep running when you detach, survive a server crash, and
reattaching works over SSH.

The rule Monarch is built around: **whatever runs inside a pane behaves exactly as it would in a
bare terminal.** Keys, mouse, paste, colours and terminal queries all pass through unchanged.

> **Status: pre-release.** Monarch is usable day to day on macOS and Linux: panes, tabs, agent
> states, the attention inbox, worktrees, review, restore and themes all work. There is no tagged
> release yet, so install from source. See [`docs/plan.md`](docs/plan.md) for progress.

## Install

From source (needs a stable Rust toolchain, 1.88 or newer, and nothing else):

```sh
cargo install --git https://github.com/cooksterj/monarch   # latest main
# or, from a clone:
cargo install --path .
```

Then, once:

```sh
monarch setup    # let Claude Code and Codex report their state; installs the Monarch skill
monarch doctor   # check the whole setup, with a fix for anything missing
```

`monarch setup` adds hooks to `~/.claude/settings.json` and `~/.codex/config.toml` (honouring
`CLAUDE_CONFIG_DIR` and `CODEX_HOME`), touching nothing else and backing each file up first.
`--dry-run` shows what would change; `--uninstall` removes it all. Outside Monarch the hooks do
nothing. Codex asks you to trust new hooks in `/hooks`.

Prebuilt binaries and a Homebrew formula will come with the first release.

## Use

```sh
monarch                 # attach to the session `main`, starting it if needed
monarch attach -s work  # another session
monarch ls              # running sessions
monarch new feat/x --agent claude   # a worktree for branch feat/x, with Claude Code in it
```

Inside, everything starts with the prefix, `Ctrl-b`. Press it and wait to see every key.

| Keys | Action |
|---|---|
| `prefix %` / `prefix "` | split right / down |
| `prefix h j k l` | move focus |
| `prefix c`, `prefix 1`–`9` | new tab, go to a tab |
| `prefix a` | attention inbox: every agent waiting on you |
| `prefix Tab` | jump to the next agent waiting on a decision |
| `prefix N` | new worktree: branch name, then shell, Claude Code or Codex |
| `prefix s` | projects: every worktree |
| `prefix v` | review the focused pane's uncommitted changes |
| `prefix S` | sessions |
| `prefix [` | copy mode |
| `prefix :` | command palette |
| `prefix d` | detach |
| `prefix ?` | help: every key, mouse gesture and command |

**[The user guide](docs/guide.md)** covers all of it: agents and their states, the inbox,
worktrees and review, copy mode and the mouse, sessions and restore, every key, rebinding,
configuration and themes, and how agents can drive Monarch themselves.

## Development

```sh
just check   # fmt, clippy, file-size lint, tests
```

Read [`CLAUDE.md`](CLAUDE.md) for code conventions and [`docs/`](docs/README.md) for the design,
architecture and benchmarks.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
