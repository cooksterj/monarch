---
name: monarch
description: Use when running inside the Monarch terminal multiplexer (the MONARCH_PANE environment variable is set) to see what the agents in other panes are doing, wait for one to finish or to need input, or start another agent in its own git worktree and give it a task.
---

# Working inside Monarch

Monarch is a terminal multiplexer for coding agents. It shows every pane's agent
state in its sidebar, and its `monarch` command lets you ask about those agents from
your shell.

## Am I inside Monarch?

Check the environment:

- `MONARCH_PANE` is set: you are in a Monarch pane. Its value is your own pane id
  (`p3`). Never wait on your own pane.
- `MONARCH_SESSION` names your session. Every `monarch agent …` command defaults to it.

If `MONARCH_PANE` is unset, you are not in Monarch: don't use this skill.

If `command -v monarch` finds nothing, the binary isn't on `PATH`. Ask the user where
it is; don't guess.

## See what every pane is doing

```sh
monarch agent explain
```

prints one line per pane:

```text
p1   tab 1  zsh     $ not an agent
p2   tab 2  claude  ● Working  (from its hooks)
p3   tab 2  gemini  ◆ Blocked  (from its screen: apply_or_allow_change)
```

States are `Working`, `Blocked` (waiting for a person: a permission prompt or a
question), `Idle`, `Done` (finished a turn nobody has looked at yet), `Error` and
`Exited`. The part in brackets says where the state came from:

- **its hooks:** the agent reports its own state. This is the most reliable source.
- **its screen:** read from the pane with a detection manifest.
- **a terminal notification:** the agent sent one.
- **Monarch launching it:** nothing has reported since it started.

For one pane in detail, including every detection rule and the text it matched:

```sh
monarch agent explain p3
```

## Wait for another agent

```sh
monarch agent wait p3 --timeout 600
```

blocks until pane `p3` needs a person (`Blocked`, `Done` or `Error`), then prints its
status, for example `p3 ◆ Blocked: Allow Bash: cargo test?`.

- Choose the states with `--until`:
  ```sh
  monarch agent wait p3 --until done,idle --timeout 600
  ```
- Always pass `--timeout` (in seconds). Without it the command waits forever, and
  your tool call can hang.
- Exit status:
  - **0:** reached one of the states.
  - **2:** timed out; it prints the state at that moment.
  - **1:** error, for example no such pane, or the pane exited.
- A state that comes and goes between checks still counts. A short `Blocked` won't
  be missed.
- When an agent is `Blocked`, tell the user which pane needs them. Don't answer
  another agent's permission prompt yourself.

## Start another agent on a branch

Run this from inside the repository:

```sh
monarch new feat-parser --agent claude --detached
```

This creates (or reuses) a git worktree for branch `feat-parser` beside the
repository and opens a tab in your session with Claude Code running there. `--agent`
takes `claude` or `codex`. It prints the new pane:

```text
worktree /src/app-worktrees/feat-parser on new branch feat-parser; opened in pane p4 of session main
```

The new agent starts with an empty prompt. Give it its task with `monarch pane send`
(below), then follow it with `monarch agent wait p4`.

## Give an agent its task

Wait until the new agent is ready for input, then type its prompt and press Enter:

```sh
monarch agent wait p4 --until idle --timeout 120
monarch pane send p4 "Add a parser for the config format. Run cargo test before you stop." --enter
monarch agent wait p4 --timeout 1800
```

- For a long or multi-line prompt, write it to a file and send it on standard input:
  ```sh
  monarch pane send p4 --enter < task.md
  ```
  It arrives as one paste, so its line breaks don't submit it early.
- Leave out `--enter` to type without submitting.
- `monarch pane send` types into any pane, a shell too. Only send to panes you
  started, or that the user asked you to use. You can't send to your own pane.
- Don't use it to answer another agent's permission prompt. Tell the user which pane
  needs them.

## Other commands

- `monarch ls`: lists running sessions.
- `monarch keys`: lists the key bindings, for telling the user how to switch tabs or
  panes.

Never run these unless the user asks:

- `monarch kill`: ends a whole session.
- `monarch attach`: takes over the terminal.
- `monarch server` and `monarch hook`: used by Monarch itself.
