# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The product is a terminal application. `web` applies to the surfaces designed with this record: the GitHub README and any page about Monarch.

## Users

Developers running several AI coding agents (Claude Code, Codex and others) at once, from a terminal they already like, often over SSH. Many are tmux veterans. Their job is to keep several agents moving without losing track of which one is stuck waiting on them.

## Product Purpose

Monarch is a terminal multiplexer that shows which agents are working, which are blocked on you and which are done, grouped by git repository and worktree. Success is never hunting through panes for the stuck agent, and never noticing Monarch got in the way of a program.

## Positioning

The lead claim is attention: you always see which agent needs you. The proof that separates it from other agent-aware multiplexers is fidelity: whatever runs in a pane behaves as it would in a bare terminal, checked by tests that compare the bytes. Agent state comes from the agent's own hooks first, then terminal notifications, then the screen.

## Operating Context

Runs inside the user's existing terminal (it never impersonates it) with a server that owns PTYs and emulators, on macOS and Linux. Keys are prefix-only, default `Ctrl-b`, fully rebindable, with monarch, tmux and herdr presets.

## Capabilities and Constraints

Shipped: panes, tabs, sidebar grouped by repo and worktree with git badges, agent states, attention inbox with answer-in-place, worktree creation, built-in diff review, copy mode, mouse, detach and reattach, restore after a crash with agent resume, themes, a CLI agents can drive.

Not shipped, and must not be implied: a tagged release or prebuilt binaries (install is from source), a plugin system, Windows support, a multi-machine view, a web client.

## Brand Commitments

Name Monarch; package `monarch-mux`; binary `monarch`; licensed MIT OR Apache-2.0. The identity is the monarch butterfly: wing orange `#e8761b`, edge `#1e1612`, spot `#f5f0e6` (theme tokens `wing`, `wing_edge`, `wing_spot`), and the butterfly sprite on the welcome screen, which the logo is drawn from.

## Evidence on Hand

- The real binary, which can be captured directly. Captures use scripted agents and say so.
- Measured performance in `docs/bench/m6-twenty-panes.md` and `docs/bench/spike4.md`.
- Terminal fidelity guarantees in `docs/architecture/terminal-core.md`.
- Detection manifests for 22 agents, vendored from Herdr with attribution in `crates/monarch-detect/manifests/ATTRIBUTION.md`.
- No users, testimonials, stars or press to cite. Do not imply any.

## Product Principles

1. UX is paramount, but the terminal experience must never be degraded.
2. Comparisons with other tools are candid, sourced, and show where Monarch loses.
3. Say what is true today: pre-release is stated plainly, and absent features are named.
