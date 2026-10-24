//! What a pane's terminal needs to know about the host to answer queries.

use crate::Palette;

/// Host terminal facts probed by the client at attach time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProfile {
    /// Host colours, for OSC 4/10/11/12 replies.
    pub palette: Palette,
    /// Cell width in pixels, for XTWINOPS text-area size replies.
    pub cell_width_px: u16,
    /// Cell height in pixels.
    pub cell_height_px: u16,
}

impl Default for HostProfile {
    fn default() -> Self {
        Self {
            palette: Palette::default(),
            cell_width_px: 8,
            cell_height_px: 16,
        }
    }
}
