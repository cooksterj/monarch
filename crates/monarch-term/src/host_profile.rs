//! What a pane's terminal needs to know about the host to answer queries.

use monarch_proto::HostInfo;

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

impl From<&HostInfo> for HostProfile {
    /// Takes every fact the host answered, keeping defaults for the rest.
    fn from(info: &HostInfo) -> Self {
        let mut profile = Self::default();
        let palette = &mut profile.palette;
        palette.foreground = info.foreground.unwrap_or(palette.foreground);
        palette.background = info.background.unwrap_or(palette.background);
        palette.cursor = info.cursor.unwrap_or(palette.cursor);
        if let Some((w, h)) = info.cell_px.filter(|&(w, h)| w > 0 && h > 0) {
            profile.cell_width_px = w;
            profile.cell_height_px = h;
        }
        profile
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::Rgb;

    use super::*;

    #[test]
    fn host_answers_override_defaults() {
        let bg = Rgb::new(0xfa, 0xfa, 0xf0);
        let info = HostInfo {
            background: Some(bg),
            cell_px: Some((10, 21)),
            ..HostInfo::default()
        };
        let profile = HostProfile::from(&info);
        assert_eq!(profile.palette.background, bg);
        assert_eq!((profile.cell_width_px, profile.cell_height_px), (10, 21));
        assert_eq!(profile.palette.foreground, Palette::default().foreground);
    }

    #[test]
    fn zero_cell_size_is_ignored() {
        let info = HostInfo {
            cell_px: Some((0, 0)),
            ..HostInfo::default()
        };
        assert_eq!(HostProfile::from(&info), HostProfile::default());
    }
}
