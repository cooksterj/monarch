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
monarch setup           # let Claude Code and Codex report their state to Monarch
monarch new feat/x --agent claude   # worktree for branch feat/x, Claude Code working in it
monarch ls              # running sessions
monarch kill work       # end the session named work (it won't be restored)
monarch agent explain   # every pane's agent state and where it came from
monarch agent wait p3 --timeout 600   # until pane p3's agent needs you (blocked, done, error)
```

`monarch new <branch>` adds a git worktree for the branch (created from the current commit, or
`--base <ref>`, unless it exists) beside the repository as `<repo>-worktrees/<branch>` (or under
`[projects].worktree_root`), and opens a tab there running `--agent claude|codex` or a shell. Run
it inside Monarch and the tab just appears; outside, it attaches.

`monarch setup` adds Monarch's hooks to `~/.claude/settings.json` and `~/.codex/config.toml`
(honouring `CLAUDE_CONFIG_DIR` and `CODEX_HOME`), touching nothing else and backing each file up
first. It is safe to re-run; `--dry-run` shows what would change and `--uninstall` removes the
hooks. Outside Monarch the hooks do nothing. Codex asks you to trust new hooks in `/hooks`.

Agents can use these commands too. `skills/monarch/SKILL.md` is a Claude Code skill that teaches
an agent inside Monarch to check on other panes, wait for them and start agents in worktrees.
Install it for yourself with
`mkdir -p ~/.claude/skills/monarch && cp skills/monarch/SKILL.md ~/.claude/skills/monarch/`.

Configuration lives at `~/.config/monarch/config.toml` (override with `MONARCH_CONFIG`).
`monarch config default` prints every setting; `monarch config check` validates your file.
`monarch doctor` checks the whole setup (config, agent hooks, git, the terminal, running
sessions) and prints a fix for anything missing.

### Themes

Monarch colours only its own chrome: the status bar, pane borders, the sidebar and pop-ups. What
runs in a pane keeps its own colours. By default the theme follows your terminal's background:
`monarch-dark` on dark backgrounds and `monarch-light` on light ones. Built-in themes are
`monarch-dark`, `monarch-light`, `catppuccin-mocha`, `catppuccin-latte`, `tokyonight-night`,
`tokyonight-day`, `gruvbox-dark` and `gruvbox-light`.

```toml
[theme]
auto_switch = true            # pick `dark` or `light` from the terminal's background
dark = "catppuccin-mocha"
light = "catppuccin-latte"
# auto_switch = false
# name = "gruvbox-dark"       # always use this one

[theme.colors]                # override single tokens, as #rrggbb
accent = "#ff8800"
```

The tokens are `bar`, `bar_text`, `accent`, `accent_text`, `working`, `blocked_from`, `blocked_to`,
`done_from`, `done_to`, `error`, `added`, `removed` and `hunk`. An unknown name or a bad colour
is reported and the default is used instead. Changes apply when you save the file. With `NO_COLOR`
set, Monarch draws its chrome with reverse video and bold instead of colours.

## Keys

The first time you attach, a welcome screen shows the handful of keys you need; it is in the
command palette as `welcome` if you want it again.

The sidebar groups panes by repository and worktree, with a badge for uncommitted files and
commits ahead of or behind upstream (`±3 ↑2 ↓1`).

Monarch uses a tmux-style prefix, `Ctrl-b` by default, so it never takes keys away from the
programs running inside it. Press the prefix and wait briefly to see every available key.

| Keys | Action |
|---|---|
| `prefix %` / `prefix "` | split right / down |
| `prefix h j k l` | move focus |
| `prefix a` | attention inbox: every agent waiting on you |
| `prefix Tab` | jump to the next agent waiting on a decision |
| `prefix v` | review the focused pane's uncommitted changes |
| `prefix N` | new worktree: branch name, then shell, Claude Code or Codex |
| `prefix s` | projects: every worktree; Enter opens it, Ctrl-x twice removes it |
| `prefix S` | sessions: switch to another, or type a new name to start one |
| `prefix :` | command palette |
| `prefix d` | detach |
| `prefix ?` | help: every key (from your own keymap), mouse gesture and command; type to search |

## Development

```sh
just check   # fmt, clippy, file-size lint, tests
```

Read [`CLAUDE.md`](CLAUDE.md) for code conventions and [`docs/plan.md`](docs/plan.md) for the
design and progress.

## License

Licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
