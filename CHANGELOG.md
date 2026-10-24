# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cargo workspace: `monarch-core`, `monarch-proto`, `monarch-term`, `monarch-detect`,
  `monarch-server`, `monarch-tui`, `monarch-cli`, with the installable `monarch` binary.
- Configuration file (`~/.config/monarch/config.toml`) with defaults for every section; unknown
  keys are warnings, not errors.
- `monarch config path|default|check` and a welcome screen.
- CI (fmt, clippy, file-size lint, tests on macOS and Linux) and a cargo-dist release pipeline.
