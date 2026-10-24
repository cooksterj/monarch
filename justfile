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
    bash scripts/lint-size.sh

# Licenses and advisories (needs cargo-deny).
deny:
    cargo deny check

# Install the local build onto PATH.
install:
    cargo install --path . --locked

# Headless PTY harnesses for the M1 spike prototype (fidelity checks).
smoke:
    cargo build -p monarch-mux-tui --example single_pane
    python3 scripts/spike1_smoke.py target/debug/examples/single_pane
    python3 scripts/spike3_input_matrix.py target/debug/examples/single_pane
    cargo build --bin monarch
    python3 scripts/spike4_attach_smoke.py target/debug/monarch
    python3 scripts/spike5_mouse_smoke.py target/debug/monarch
    python3 scripts/spike6_restore_smoke.py target/debug/monarch

# M1 spike 4: latency/load bench (release build; prints a table, non-zero on failure)
bench-spike4 *ARGS:
    cargo build --release --bin monarch
    cargo build --release -p monarch-mux-tui --example spike4_bench
    target/release/examples/spike4_bench target/release/monarch {{ARGS}}
