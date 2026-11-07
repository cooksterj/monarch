# Monarch user guide

Monarch runs your shells and coding agents (Claude Code, Codex) in panes inside the terminal you
already use, keeps them running when you detach, and shows at a glance which agents are working,
which are waiting on you, and which have finished.

What runs inside a pane behaves as it would in a bare terminal. Monarch never takes a key, mouse
gesture or paste away from a program without the prefix first.

- [Getting started](#getting-started)
- [The screen](#the-screen)
- [Panes, tabs and layout](#panes-tabs-and-layout)
- [Agents](#agents)
- [Projects and worktrees](#projects-and-worktrees)
- [Copy mode and the mouse](#copy-mode-and-the-mouse)
- [Sessions, detaching and restore](#sessions-detaching-and-restore)
- [Keys](#keys)
- [Configuration](#configuration)
- [Agents driving Monarch](#agents-driving-monarch)
- [Troubleshooting](#troubleshooting)

## Getting started

Install it with Homebrew, the shell installer or from source (see the [README](../README.md#install)), then:

```sh
monarch setup    # once: let Claude Code and Codex report their state, install the skill
monarch doctor   # check everything is in place
monarch          # attach to the session named main, starting it if needed
monarch concept  # or to a session of your own: here, concept
```

The first time you attach, a welcome screen shows the few keys you need. Any key closes it. It is
in the command palette (`prefix :`, then `welcome`) if you want it again.

Everything starts with the **prefix**, `Ctrl-b` by default. Press it, let go, then press the key
for the action. Wait a moment after the prefix and a pop-up lists every key you can press next.
`prefix ?` opens the full help page, built from your own keymap.

## The screen

- **Sidebar** (left): every pane, grouped by git repository and worktree, each with its agent's
  state. After each branch, a badge says how the checkout differs (nothing when it is clean
  and in step with its upstream):

  | Badge | Colour | Meaning |
  |---|---|---|
  | `+12` | green | lines added since the last commit, staged or not |
  | `−4` | red | lines removed |
  | `?2` | dim | new files git doesn't track yet (their lines aren't in `+`) |
  | `↑2` | accent | commits to push |
  | `↓1` | amber | commits to pull; with `↑` too, the branch has diverged |

  `prefix t` toggles the sidebar; it hides itself when the terminal is narrower than
  `ui.sidebar_min_columns`. Repositories are listed alphabetically (then by branch), with panes
  outside any repository last under `Scratch`. Click a row to focus that pane. When there are
  more panes than fit, the wheel over the sidebar scrolls it, a thumb on its right edge shows
  where you are, and focusing a pane whose row is out of sight brings it into view.
- **Panes**: the rest of the screen, split as you like.
- **Status line** (bottom): the session, its tabs, whether Monarch or the program has the mouse,
  and a key hint. After the prefix it shows that Monarch is waiting for the next key.

## Panes, tabs and layout

| Keys after the prefix | Action |
|---|---|
| `%` or `\|` | split right |
| `"` or `-` | split down |
| `h j k l` or arrows | focus the pane in that direction |
| `o` | focus the next pane |
| `g` or `w` | go to a pane by name (a searchable list) |
| `H J K L` | move the divider that way |
| `=` | make the panes equal |
| `z` | zoom the focused pane (again to unzoom) |
| `{` `}` | swap with the previous / next pane |
| `x` | close the pane |
| `c` | new tab |
| `n` `p` | next / previous tab |
| `1`–`9` | go to that tab |
| `,` | rename the tab. A tab with one pane shows the name in the sidebar too; in a split tab each pane keeps its own name until you name it with `.`. A blank name gives back the automatic one (the program's) |
| `.` | rename the focused pane, so the panes of a split tab can be told apart; a pane's own name wins over its tab's, and a blank name gives the automatic one back |

From a shell, `monarch tab rename api` names the tab it runs in (outside Monarch: the tab shown),
`--tab 2` names another, and `monarch tab rename ""` gives the automatic name back.

Focus and resize keys repeat: within half a second (`keys.repeat_ms`) you can press them again
without the prefix, so `prefix L L L` grows a pane three steps.

## Agents

Each pane running an agent shows its state in the sidebar and the inbox:

| Glyph | State | Meaning |
|---|---|---|
| `●` | Working | busy on your last request |
| `◆` | Blocked | waiting for you: a permission prompt or a question |
| `○` | Idle | ready for a new request |
| `✓` | Done | finished a turn you haven't looked at yet: bright green, and listed in the inbox |
| `✓` | Finished | a Done you've since focused: soft green, no longer asking for you |
| `✗` | Error | the turn failed |

At the right of each agent's row is a stopwatch of the turn's working time: `42s`, then `3m`,
`2h`, `1d`. It runs while the agent works, holds while the agent waits on you for a permission
or an answer, runs on once you reply, and stops when the turn ends, staying there while the
agent rests. How long a prompt has been waiting on you is the inbox's job (below). A turn that
ends in the pane you are looking at is Finished at once rather than Done.
| `■` | Exited | the agent's process ended |
| `$` | (none) | a plain shell |

After the state, an agent's icon says which agent it is: `✻` Claude Code (terracotta, the
`claude` token) and `⌬` Codex (the `codex` token, your theme's text colour). The tab bar shows the
icon of each tab's focused pane, so `1 ✻ api` is a Claude tab even after you rename it.

States come from, in order of trust: the agent's own **hooks** (installed by `monarch setup`),
terminal **notifications** the agent sends, and its **screen**, read with detection rules for
about twenty agents. `monarch agent explain` shows each pane's state and where it came from.

**Start an agent** in a new worktree with `prefix N` (below), or in a new session with
`monarch attach -s work --agent claude`.

**The inbox**, `prefix a`, lists every agent waiting on you with the question it asked or the
last thing it said, and at the right its state and how long it has waited (`blocked 3m`), or
for a finished turn how long it worked (`done in 45s`). Blocked agents come first, then errors,
then done; within each, the one that has been so longest is at the top. Move with `j`/`k` or the arrows; `Enter` jumps to that pane. Any other letter
or digit answers that agent without leaving the inbox. `y` and `n` are translated for the agent
on that row: Claude Code's permission prompt gets `1` and Esc, Codex gets `y` and `n`, as
`[agents.<id>].quick_answers` says. Every other key is typed as it is, so `2` still picks a
prompt's second option. The status line says what was sent.

**`prefix Tab`** jumps straight to the next blocked agent.

## Projects and worktrees

Monarch groups panes by the repository and worktree of their working directory; there is
nothing to set up.

**A new worktree**, `prefix N`: type a branch name, choose shell, Claude Code or Codex with `Tab`
or `←` `→`, and press `Enter`. Monarch checks the branch out in a new worktree (creating the
branch from the current commit if it doesn't exist) and opens a tab there. Worktrees go beside the
repository as `<repo>-worktrees/<branch>`, or under `projects.worktree_root`. From a shell:

```sh
monarch new feat/parser --agent claude      # --base main to branch from elsewhere
```

Run inside Monarch, the tab just appears; outside, it attaches.

**Projects**, `prefix s`: every worktree of every repository open in the session. `Enter` opens
one (focusing its pane, or opening a tab there); `Ctrl-x` twice removes a worktree.

**Review**, `prefix v`: the focused pane's uncommitted changes, file by file with a coloured
diff. When the pane runs an agent, `c` writes a note on the hunk at the top of the diff and
sends it to that agent: the file, the hunk header, your words, and the hunk itself quoted as a
diff, typed into the pane as one paste and submitted. The review stays open, so you can keep
reading and send the next one. Move the hunk you mean to the top with `[` and `]`, or a line at
a time with `Ctrl-e` and `Ctrl-y`.

| Keys | Action |
|---|---|
| `j` `k` `Tab` | next / previous file |
| `Space` `PgDn` `Ctrl-d` | scroll down |
| `b` `PgUp` `Ctrl-u` | scroll up |
| `Ctrl-e` `Ctrl-y` | scroll one line |
| `[` `]` | previous / next hunk |
| `g` `G` | top / bottom |
| `c` | note on the hunk shown, sent to the pane's agent |
| `r` | refresh |
| `e` or `Enter` | open the file in `$EDITOR`, zoomed over the pane |
| `q` or `Esc` | close |

## Copy mode and the mouse

**Mouse.** Click focuses a pane. Drag selects text and copies it when you let go;
double-click selects a word and triple-click a line. The wheel scrolls the pane's history.
Right-click opens a pane and tab menu. When the program in the pane uses the mouse itself (vim,
htop), clicks go to it; hold Shift to select anyway. `prefix m` turns Monarch's mouse handling off
and on, leaving selection to your terminal. With `ui.mouse = "pane"`, Monarch only takes the mouse
while the focused program asks for it.

Copying uses OSC 52, which works over SSH; your terminal may need clipboard access enabled.

**Copy mode**, `prefix [`, moves a cursor through the pane and its history with vi keys:

| Keys | Action |
|---|---|
| `h j k l`, arrows | move |
| `w b e` | by word |
| `0` `^` `$`, `Home` `End` | start, first character, end of line |
| `H M L` | top, middle, bottom of the screen |
| `Ctrl-d` `Ctrl-u` | half a page |
| `Ctrl-f` `Ctrl-b`, `PgDn` `PgUp` | a page |
| `g` `G` | top of history / bottom |
| `v` or `Space`, `V`, `Ctrl-v` | select characters, lines, a block |
| `y` or `Enter` | copy the selection and leave |
| `/` `?`, then `n` `N` | search down / up, repeat |
| `q`, `Ctrl-c`, `Esc` | leave (`Esc` first clears a selection) |

## Sessions, detaching and restore

A session is one server holding tabs and panes. It keeps running when you detach (`prefix d`) or
close the terminal; `monarch` attaches again, from anywhere, over SSH too. Two terminals can be
attached at once.

- `prefix S` lists sessions; pick one to switch, or type a new name to start it.
- `monarch work` attaches to (or starts) the session `work`. A name that is also a command
  needs the long form, `monarch attach -s ls`, and so does a name one letter away from a command
  (`monarch doctr` asks whether you meant `doctor` rather than starting a session).
- `monarch ls` lists running sessions; `monarch kill work` ends one for good.

Monarch saves each session's layout as it changes. If the server dies (a crash, a reboot,
`kill -9`), the next `monarch` brings back the tabs, splits and each pane's working directory,
and resumes the conversations of agents it started (`--agent`, `monarch new --agent`, `prefix N`):
Claude Code with `claude --resume`, Codex with `codex resume` (its own picker if Codex's hooks
never reported a session id). An agent you started by hand comes back as a shell in the same
directory. Running programs and their screens can't survive the server, as with tmux.
`monarch kill` is the way to end a session without it coming back.

## Keys

`monarch keys` prints every binding. Besides the keys above:

| Keys after the prefix | Action |
|---|---|
| `a` | inbox |
| `Tab` | next blocked agent |
| `N` | new worktree |
| `s` | projects |
| `S` | sessions |
| `v` | review |
| `:` | command palette: every action by name |
| `?` | help |
| `[` | copy mode |
| `d` | detach |
| `m` | mouse on / off |
| `t` | sidebar on / off |
| `R` | reload the config |

In every pop-up: type to search; `↑` `↓`, `Tab` or `Ctrl-n` `Ctrl-p` move; `Enter` chooses;
`Esc`, `Ctrl-c` or `Ctrl-g` closes; `Ctrl-w` and `Ctrl-u` delete a word or the whole query.

**Presets.** `keys.preset` picks `monarch` (the default, above), `tmux` (tmux's own defaults,
tabs numbered from 0) or `herdr`.

**Rebinding.** Bindings in `[keys.bind]` are laid over the preset; an action name binds a key,
`"none"` removes one. Action names are in `monarch keys` and the help page.

```toml
[keys]
prefix = "ctrl+a"          # any single control key

[keys.bind]
"v" = "split-right"
"x" = "none"               # no accidental closes
```

## Configuration

The file is `~/.config/monarch/config.toml` (`MONARCH_CONFIG` overrides it; `monarch config path`
prints it). It is optional: `monarch config default` prints every setting with its default.
Changes apply when you save; `prefix R` reloads by hand. A bad value is reported and its default
used, never a crash. `monarch config check` validates the file and flags unknown keys. The
settings a server reads when it starts panes (`[agents.*]`, `[terminal]`, `session.resume_agents`)
take effect for the next session you start; a running server keeps the values it started with.

| Setting | Default | Meaning |
|---|---|---|
| `keys.prefix` | `"ctrl+b"` | the prefix |
| `keys.preset` | `"monarch"` | `monarch`, `tmux` or `herdr` |
| `keys.repeat_ms` | `500` | how long repeatable keys work without the prefix |
| `keys.which_key_delay_ms` | `300` | delay before the key pop-up appears |
| `keys.bind` | | your own bindings, as above |
| `ui.sidebar` | `true` | show the sidebar |
| `ui.sidebar_width` | `32` | its width in columns |
| `ui.sidebar_min_columns` | `100` | hide it in terminals narrower than this |
| `ui.mouse` | `"auto"` | `auto`, or `pane` to take the mouse only when the program wants it |
| `ui.window_title` | `"{title} — monarch:{session}"` | your terminal's title; `{session}`, `{pane}`, `{title}` expand |
| `ui.animations` | `true` | animate state glyphs |
| `terminal.shell` | `$SHELL` | shell for new panes |
| `terminal.scrollback` | `10000` | history lines per shell pane |
| `terminal.agent_scrollback` | `5000` | history lines per pane Monarch started as an agent (they redraw a lot) |
| `terminal.env` | `{}` | extra environment for every pane, e.g. `{ EDITOR = "nvim" }`; Monarch's own variables (`TERM`, `TERM_PROGRAM`, `MONARCH_*`) always win |
| `session.default_name` | `"main"` | the session `monarch` attaches to |
| `session.resume_agents` | `true` | restore relaunches agents; off, their panes come back as shells in the same directory |
| `projects.worktree_root` | beside the repo | where new worktrees go |
| `agents.<id>.command` | `claude` / `codex` | the program `--agent`, `monarch new --agent` and `prefix N` run; a wrapper script works and is recognised by name |
| `agents.<id>.args` | `["--session-id", "{session_id}"]` / `[]` | its arguments for a new session; `{session_id}` makes Monarch choose a UUID so the pane can be resumed |
| `agents.<id>.resume_args` | `["--resume", "{session_id}"]` / `["resume", "{session_id}"]` | arguments to resume after a restart; an argument naming `{session_id}` is left out when no id is known (Codex before its hooks report one); empty means start a new session instead |
| `agents.<id>.quick_answers` | `y`/`n` per agent | what an inbox key sends to that agent's pane, e.g. `y = "1"` |

To add a default flag to every agent Monarch starts, keep the placeholder:

```toml
[agents.claude]
args = ["--model", "opus", "--session-id", "{session_id}"]
```

Only `claude` and `codex` can be configured; any other `[agents.<id>]` table is reported and
ignored, as is an empty `command` (the built-in one is used).

### Themes

Monarch colours only its own chrome; programs in panes keep their colours. By default the theme
follows your terminal's background: `monarch-dark` or `monarch-light`. The built-ins are
`monarch-dark`, `monarch-light`, `catppuccin-mocha`, `catppuccin-latte`, `tokyonight-night`,
`tokyonight-day`, `gruvbox-dark` and `gruvbox-light`.

```toml
[theme]
auto_switch = true            # choose `dark` or `light` from the terminal's background
dark = "catppuccin-mocha"
light = "catppuccin-latte"
# auto_switch = false
# name = "gruvbox-dark"       # always this one

[theme.colors]                # override single tokens, as #rrggbb
accent = "#ff8800"
```

Tokens: `bar`, `bar_text`, `accent`, `accent_text`, `working`, `blocked_from`, `blocked_to`,
`done_from`, `done_to`, `error`, `added`, `removed`, `hunk`, `claude`, `codex`, `wing`,
`wing_edge`, `wing_spot`.
With `NO_COLOR` set, Monarch draws with reverse video and bold instead.

## Agents driving Monarch

Agents in Monarch panes can watch and start other agents with the same commands you use. Every
pane has `MONARCH_PANE` (its own id, like `p3`) and `MONARCH_SESSION` set, and the commands default
to that session.

```sh
monarch agent explain                          # every pane's state, and why
monarch agent wait p3 --timeout 600            # until p3 needs a person (blocked, done, error)
monarch agent read p3                          # what it said when it stopped, in full
monarch new feat-parser --agent claude --detached
monarch pane send p4 "Add a parser. Run cargo test before you stop." --enter
monarch pane send p4 --enter < task.md         # a long prompt, as one paste
monarch tab rename parser --tab 2              # name the tab it opened
```

`monarch agent wait` exits 0 when the state is reached and 2 on timeout. `monarch agent read`
prints the agent's last message of its turn as its hooks delivered it (for an agent without hooks,
what its screen shows), and exits 2 when it has said nothing yet. `monarch pane send` types as if
you had; multi-line text arrives as one bracketed paste, so line breaks don't submit it early. It
refuses the pane it runs in.

`monarch setup` installs a skill that teaches Claude Code and Codex these commands
(`~/.claude/skills/monarch/`, `~/.agents/skills/monarch/`).

## Troubleshooting

`monarch doctor` checks the binary on `PATH`, the config, each agent's hooks and skill, git, the
terminal's colour support and running sessions, and prints a fix for anything wrong.

- **An agent's state never changes.** Run `monarch setup`, then restart the agent. Codex asks
  you to trust new hooks in its `/hooks` screen. `monarch agent explain p3` shows what Monarch
  last heard.
- **Clicks do nothing.** Check that your terminal's own mouse reporting is on (iTerm2 toggles it
  per tab with `Cmd-Opt-R`), and that `prefix m` hasn't turned Monarch's off.
- **Copying doesn't reach the clipboard.** Allow OSC 52 clipboard access in your terminal's
  settings.
- **`monarch setup` changed something you didn't want.** `monarch setup --uninstall` removes the
  hooks and skill; each edited file was backed up first.
