//! `monarch keys`: what each key after the prefix does.

use std::io::Write;
use std::process::ExitCode;

use monarch_core::{Config, Keymap};

/// Prints the configured bindings, one per line, then any config warnings.
pub(crate) fn run(config: &Config, out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let (keymap, warnings) = Keymap::from_config(&config.keys);
    writeln!(
        out,
        "prefix {} ({} preset), then:",
        config.keys.prefix, config.keys.preset
    )?;
    for (key, action) in keymap.bindings() {
        writeln!(out, "  {:<8}{action}", key.to_string())?;
    }
    for warning in warnings {
        writeln!(out, "warning: {warning}")?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(config: &Config) -> String {
        let mut out = Vec::new();
        run(config, &mut out).expect("runs");
        String::from_utf8(out).expect("utf8")
    }

    #[test]
    fn lists_bindings_and_names_the_preset() {
        let text = keys(&Config::default());
        assert!(text.starts_with("prefix ctrl+b (monarch preset)"), "{text}");
        assert!(text.contains("  %       split-right\n"), "{text}");
        assert!(text.contains("  left    focus-left\n"), "{text}");
    }

    #[test]
    fn bad_bindings_are_reported() {
        let mut config = Config::default();
        config.keys.bind.insert("v".into(), "explode".into());
        assert!(keys(&config).contains("warning: [keys.bind]"));
    }
}
