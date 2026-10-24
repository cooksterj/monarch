//! What the client learned about the host terminal by probing it.

use serde::{Deserialize, Serialize};

use crate::Rgb;

/// Host terminal facts, probed by the client at attach time and sent to the
/// server so pane queries (OSC 10/11/12, `CSI 14/16 t`) are answered without
/// a round trip. `None` means the host did not answer that probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HostInfo {
    /// Default foreground (OSC 10).
    pub foreground: Option<Rgb>,
    /// Default background (OSC 11).
    pub background: Option<Rgb>,
    /// Cursor colour (OSC 12).
    pub cursor: Option<Rgb>,
    /// Cell size in pixels, width then height (`CSI 16 t`).
    pub cell_px: Option<(u16, u16)>,
    /// Whether the host supports synchronized output (DECRQM 2026).
    pub sync_output: bool,
    /// The host's current kitty keyboard flags; `Some` means the host speaks
    /// the kitty keyboard protocol (`CSI ? u`).
    pub kitty_flags: Option<u8>,
}

impl HostInfo {
    /// Whether the host understands the kitty keyboard protocol.
    #[must_use]
    pub fn kitty_keyboard(&self) -> bool {
        self.kitty_flags.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_assumes_nothing() {
        let info = HostInfo::default();
        assert!(!info.kitty_keyboard());
        assert!(!info.sync_output);
        assert_eq!(info.background, None);
    }

    #[test]
    fn kitty_support_is_independent_of_current_flags() {
        let info = HostInfo {
            kitty_flags: Some(0),
            ..HostInfo::default()
        };
        assert!(info.kitty_keyboard());
    }

    #[test]
    fn round_trips_through_serde() {
        let info = HostInfo {
            background: Some(Rgb::new(1, 2, 3)),
            cell_px: Some((9, 18)),
            ..HostInfo::default()
        };
        let json = serde_json::to_string(&info).expect("serialize");
        assert_eq!(
            serde_json::from_str::<HostInfo>(&json).expect("parse"),
            info
        );
    }
}
