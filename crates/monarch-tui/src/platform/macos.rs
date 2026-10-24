//! macOS: the clipboard is `pbcopy`.

use std::process::Command;

use super::LocalClipboard;

/// The macOS pasteboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClipboard;

impl LocalClipboard for SystemClipboard {
    fn command(&self) -> Option<Command> {
        Some(Command::new("pbcopy"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_pbcopy() {
        let cmd = SystemClipboard.command().expect("command");
        assert_eq!(cmd.get_program(), "pbcopy");
    }
}
