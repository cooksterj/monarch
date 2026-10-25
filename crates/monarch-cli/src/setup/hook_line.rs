//! The command line an agent's hook runs, and recognising one Monarch
//! installed before (whatever path it was installed from).

use std::path::Path;

use crate::HookCommand;

/// `<monarch> hook <agent>`, with the binary quoted for the shell if it
/// needs to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HookLine {
    agent: HookCommand,
    text: String,
}

impl HookLine {
    /// The line running `program` (a path, or `monarch` when that is on
    /// `PATH`) for `agent`.
    pub fn new(program: &Path, agent: HookCommand) -> Self {
        let program = quote(&program.to_string_lossy());
        Self {
            agent,
            text: format!("{program} hook {}", agent.name()),
        }
    }

    /// The agent the line reports for.
    pub fn agent(&self) -> HookCommand {
        self.agent
    }

    /// The command text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether `command` is a Monarch hook for `agent`: a program named
    /// `monarch`, wherever it lives, run as `hook <agent>`.
    pub fn is_monarch(command: &str, agent: HookCommand) -> bool {
        let Some((program, rest)) = split_program(command.trim()) else {
            return false;
        };
        let named = Path::new(program)
            .file_name()
            .is_some_and(|n| n == "monarch");
        let args: Vec<&str> = rest.split_whitespace().collect();
        named && args == ["hook", agent.name()]
    }
}

/// The program (unquoted) and the rest of `command`.
fn split_program(command: &str) -> Option<(&str, &str)> {
    if let Some(quoted) = command.strip_prefix('\'') {
        let end = quoted.find('\'')?;
        return Some((&quoted[..end], &quoted[end + 1..]));
    }
    Some(
        command
            .split_once(char::is_whitespace)
            .unwrap_or((command, "")),
    )
}

/// `text` as one shell word.
fn quote(text: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "/._-+".contains(c);
    if !text.is_empty() && text.chars().all(plain) {
        return text.to_owned();
    }
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_name_the_program_and_agent() {
        let line = HookLine::new(Path::new("/opt/bin/monarch"), HookCommand::Codex);
        assert_eq!(line.text(), "/opt/bin/monarch hook codex");
        assert_eq!(line.agent(), HookCommand::Codex);
        let spaced = HookLine::new(Path::new("/My Apps/monarch"), HookCommand::Claude);
        assert_eq!(spaced.text(), "'/My Apps/monarch' hook claude");
        assert!(HookLine::is_monarch(spaced.text(), HookCommand::Claude));
    }

    #[test]
    fn any_monarch_install_is_recognised() {
        let claude = HookCommand::Claude;
        assert!(HookLine::is_monarch("monarch hook claude", claude));
        assert!(HookLine::is_monarch(
            " /Users/me/src/monarch/target/debug/monarch  hook claude ",
            claude
        ));
        assert!(
            !HookLine::is_monarch("monarch hook codex", claude),
            "other agent"
        );
        assert!(!HookLine::is_monarch("monarch-notify hook claude", claude));
        assert!(!HookLine::is_monarch("monarch hook claude --x", claude));
        assert!(!HookLine::is_monarch("'unterminated hook claude", claude));
        assert!(!HookLine::is_monarch("", claude));
    }
}
