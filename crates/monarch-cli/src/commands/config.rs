//! `monarch config path|default|check`.

use std::io::Write;
use std::process::ExitCode;

use monarch_core::{Config, ConfigLoader, Keymap};

use crate::ConfigCommand;

/// Runs a `config` subcommand.
pub(crate) fn run(
    cmd: &ConfigCommand,
    loader: &ConfigLoader,
    out: &mut dyn Write,
) -> anyhow::Result<ExitCode> {
    match cmd {
        ConfigCommand::Path => writeln!(out, "{}", loader.path()?.display())?,
        ConfigCommand::Default => write!(out, "{}", Config::default_toml())?,
        ConfigCommand::Check => return check(loader, out),
    }
    Ok(ExitCode::SUCCESS)
}

fn check(loader: &ConfigLoader, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let loaded = loader.load()?;
    let path = loaded.path.display();
    if !loaded.found {
        writeln!(out, "No config at {path}; using defaults.")?;
        return Ok(ExitCode::SUCCESS);
    }
    for key in &loaded.unknown_keys {
        writeln!(out, "warning: unknown key `{key}` in {path}")?;
    }
    for warning in Keymap::from_config(&loaded.config.keys).1 {
        writeln!(out, "warning: {warning}")?;
    }
    writeln!(out, "{path}: ok")?;
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_string(cmd: &ConfigCommand, loader: &ConfigLoader) -> String {
        let mut out = Vec::new();
        run(cmd, loader, &mut out).expect("command succeeds");
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn path_prints_the_explicit_path() {
        let text = run_to_string(&ConfigCommand::Path, &ConfigLoader::at("/tmp/m.toml"));
        assert_eq!(text, "/tmp/m.toml\n");
    }

    #[test]
    fn default_prints_parseable_toml() {
        let text = run_to_string(&ConfigCommand::Default, &ConfigLoader::at("/unused"));
        assert!(text.contains("[keys]"));
    }

    #[test]
    fn check_reports_unknown_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("c.toml");
        std::fs::write(&path, "bogus = 1\n").expect("write");
        let text = run_to_string(&ConfigCommand::Check, &ConfigLoader::at(&path));
        assert!(text.contains("unknown key `bogus`"), "{text}");
        assert!(text.ends_with(": ok\n"), "{text}");
    }

    #[test]
    fn check_reports_bad_key_bindings() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("c.toml");
        std::fs::write(&path, "[keys.bind]\nv = \"explode\"\n").expect("write");
        let text = run_to_string(&ConfigCommand::Check, &ConfigLoader::at(&path));
        assert!(text.contains("warning: [keys.bind] \"v\""), "{text}");
    }

    #[test]
    fn check_fails_on_type_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("c.toml");
        std::fs::write(&path, "[ui]\nsidebar = 3\n").expect("write");
        let mut out = Vec::new();
        assert!(run(&ConfigCommand::Check, &ConfigLoader::at(&path), &mut out).is_err());
    }
}
