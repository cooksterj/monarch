//! Linux: `wl-copy` under Wayland, `xclip` under X11, nothing otherwise.

use std::process::Command;

use super::LocalClipboard;

/// The desktop clipboard, when a display server is running.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClipboard;

impl LocalClipboard for SystemClipboard {
    fn command(&self) -> Option<Command> {
        command_for(
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            std::env::var_os("DISPLAY").is_some(),
        )
    }
}

fn command_for(wayland: bool, x11: bool) -> Option<Command> {
    if wayland {
        return Some(Command::new("wl-copy"));
    }
    x11.then(|| {
        let mut cmd = Command::new("xclip");
        cmd.args(["-selection", "clipboard"]);
        cmd
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_display_servers_tool() {
        assert_eq!(
            command_for(true, true).map(|c| c.get_program().to_owned()),
            Some("wl-copy".into())
        );
        assert_eq!(
            command_for(false, true).map(|c| c.get_program().to_owned()),
            Some("xclip".into())
        );
        assert!(command_for(false, false).is_none());
    }
}
