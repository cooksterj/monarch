# Contributing

Monarch uses **trunk-based development**: `main` is always releasable, and all
work lands through short-lived branches merged within a day or two.

## Workflow

1. Branch from an up-to-date `main`: `git switch -c feat/osc-tap` (prefixes:
   `feat/`, `fix/`, `refactor/`, `docs/`, `test/`, `ci/`, `chore/`).
2. Keep the branch small: one reviewable slice. Hide unfinished features
   behind config or keep them unreachable rather than holding a long branch.
3. Run `just check` before pushing (fmt, clippy, file-size lint, tests).
4. Open a PR whose **title is a conventional commit**:
   `feat(tui): mirror kitty keyboard flags to the host`. It becomes the
   squash-merge commit on `main` and the changelog entry.
5. CI must pass (Lint, Test on macOS and Linux, MSRV, Deny, PR title). Enable
   auto-merge with `gh pr merge --auto --squash` and move on.

`main` is protected: no direct pushes, no force pushes, linear history,
squash merges only, and merged branches are deleted automatically.

## Commit types

| Type | Use for | Version bump (pre-1.0) |
|---|---|---|
| `feat` | user-visible capability | minor |
| `fix` | bug fix | patch |
| `perf`, `refactor` | no behaviour change | patch |
| `docs`, `test`, `ci`, `build`, `chore` | everything else | patch |
| `feat!` / `BREAKING CHANGE:` | breaking config or CLI change | minor (major after 1.0) |

## Releases

Releases are automatic and decoupled from merging features:

- After every merge, release-plz updates an open **`chore: release vX.Y.Z`** PR
  containing the version bump and the generated `CHANGELOG.md` entry.
- **Merging that PR is the release.** It pushes the `vX.Y.Z` tag; cargo-dist
  then builds the binaries, the shell installer and the Homebrew formula and
  attaches them to a GitHub Release.
- **crates.io publishing is off** until Monarch is mature (`git_only = true`
  in `release-plz.toml`); versions come from the git tags. Install from the
  Homebrew tap, the installer script or `cargo install --git`.
- Polish changelog wording by editing the release PR before merging.

## Code conventions

See [`CLAUDE.md`](CLAUDE.md): one type per file, ≤ 300 lines, tests in every
file, traits for shared behaviour.
