# Developer recipes. `just check` is what CI runs.

default: check

# Run the test suite (nextest if installed, else cargo test).
test:
    if command -v cargo-nextest >/dev/null; then cargo nextest run --workspace; else cargo test --workspace; fi

# Formatting, lints, file-size rule and tests: run before every commit.
check: fmt-check clippy lint-size test

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Enforce the 300-line limit on non-test source.
lint-size:
    bash scripts/lint_size.sh

# Licenses and advisories (needs cargo-deny).
deny:
    cargo deny check

# Install the local build onto PATH, then point the agents' hooks at it
# (`monarch setup`, which also adds any hook events this build reads) and
# say which running sessions still run the old build (`monarch doctor`).
install:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo install --path . --locked
    bin="${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}/bin/monarch"
    "$bin" setup
    "$bin" doctor || true

# Headless PTY harnesses that play the host terminal (scripts/smoke_*.py; CI runs them too).
smoke:
    cargo build -p monarch-mux-tui --example single_pane
    python3 scripts/smoke_single_pane.py target/debug/examples/single_pane
    python3 scripts/smoke_input_matrix.py target/debug/examples/single_pane
    cargo build --bin monarch
    python3 scripts/smoke_attach.py target/debug/monarch
    python3 scripts/smoke_mouse.py target/debug/monarch
    python3 scripts/smoke_restore.py target/debug/monarch

# M6: 20 panes through the real attach client (release build; prints a table)
bench-attach *ARGS:
    cargo build --release --bin monarch
    python3 scripts/bench_attach.py target/release/monarch {{ARGS}}

# M1 spike 4: latency/load bench (release build; prints a table, non-zero on failure)
bench-spike4 *ARGS:
    cargo build --release --bin monarch
    cargo build --release -p monarch-mux-tui --example spike4_bench
    target/release/examples/spike4_bench target/release/monarch {{ARGS}}

# Regenerate the README's images from the real binary (needs pyte, Chrome, img2webp).
readme-shots:
    cargo build --release
    python3 scripts/capture_readme.py target/release/monarch docs/assets
