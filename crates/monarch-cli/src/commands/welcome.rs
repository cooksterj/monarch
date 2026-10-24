//! The screen shown by a bare `monarch` until attaching lands in M1.

use std::io::Write;
use std::process::ExitCode;

use monarch_core::ConfigLoader;

/// Prints the welcome screen with the essential keys and config status.
pub(crate) fn run(loader: &ConfigLoader, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let loaded = loader.load()?;
    let prefix = &loaded.config.keys.prefix;
    let status = if loaded.found {
        "loaded"
    } else {
        "not found, using defaults"
    };
    writeln!(
        out,
        "monarch {version} — an attention-first multiplexer for AI coding agents\n\
         \n\
         The session server and terminal client arrive in milestone M1.\n\
         \n\
         Keys you will need (prefix {prefix}):\n\
         \x20 {prefix} %   split right      {prefix} a   attention inbox\n\
         \x20 {prefix} \"   split down       {prefix} :   command palette\n\
         \x20 {prefix} d   detach           {prefix} ?   all keys\n\
         \n\
         Config: {path} ({status})\n\
         Run `monarch --help` for commands.",
        version = env!("CARGO_PKG_VERSION"),
        path = loaded.path.display(),
    )?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_configured_prefix_and_config_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("c.toml");
        std::fs::write(&path, "[keys]\nprefix = \"ctrl+a\"\n").expect("write");
        let mut out = Vec::new();
        run(&ConfigLoader::at(&path), &mut out).expect("runs");
        let text = String::from_utf8(out).expect("utf8");
        assert!(text.contains("prefix ctrl+a"), "{text}");
        assert!(text.contains("(loaded)"), "{text}");
    }
}
