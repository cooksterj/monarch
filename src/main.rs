//! The `monarch` binary: a thin entry point that hands off to `monarch_cli`.

fn main() -> std::process::ExitCode {
    monarch_cli::run()
}
