# Monarch

An attention-first terminal multiplexer for AI coding agents.

Monarch runs Claude Code, Codex and your shells in panes inside your existing terminal, like tmux,
and keeps track of which agents are working, which are blocked on you, and which are done, grouped
by git repository and worktree. Sessions keep running when you detach, and reattaching works over
SSH.

The rule Monarch is built around: **whatever runs inside a pane behaves exactly as it would in a
bare terminal.** Keys, mouse, paste, colours and terminal queries all pass through unchanged.

> **Status: early development.** The workspace, config system and CLI skeleton are in place; the
> session server and terminal client are being built now. See [`docs/plan.md`](docs/plan.md).

## Install

Homebrew (macOS, Linux):

```sh
brew install cooksterj/monarch/monarch
```

Shell installer (prebuilt binary):

```sh
curl -fsSL https://github.com/cooksterj/monarch/releases/latest/download/monarch-mux-installer.sh | sh
```

From source (requires a stable Rust toolchain, nothing else):

```sh
cargo install --git https://github.com/cooksterj/monarch   # latest main
```

Monarch is not on crates.io yet; it will be published there once it matures.

Prebuilt binaries and installers are published from the first tagged release.

## Usage

```sh
monarch                 # attach to the default session
monarch config path     # where the config file lives
monarch config default  # print the default config
monarch config check    # validate your config and flag unknown keys
```

Configuration lives at `~/.config/monarch/config.toml` (override with `MONARCH_CONFIG`).

## Keys

Monarch uses a tmux-style prefix, `Ctrl-b` by default, so it never takes keys away from the
programs running inside it. Press the prefix and wait briefly to see every available key.

| Keys | Action |
|---|---|
| `prefix %` / `prefix "` | split right / down |
| `prefix h j k l` | move focus |
| `prefix a` | attention inbox: every agent waiting on you |
| `prefix Tab` | jump to the next blocked agent |
| `prefix :` | command palette |
| `prefix d` | detach |
| `prefix ?` | all keys |

## Development

```sh
just check   # fmt, clippy, file-size lint, tests
```

Read [`CLAUDE.md`](CLAUDE.md) for code conventions and [`docs/plan.md`](docs/plan.md) for the
design and progress.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
