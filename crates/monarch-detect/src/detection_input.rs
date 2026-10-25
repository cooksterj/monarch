//! What a manifest reads: the screen's text and the pane's latest OSC
//! title and progress.

/// One pane's state as text. `screen` is the bottom of the buffer (what
/// the program last drew), one line per row, never the scrolled
/// viewport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DetectionInput<'a> {
    /// The screen, rows joined with `\n`.
    pub screen: &'a str,
    /// The window title the program set (OSC 0/2), or empty.
    pub osc_title: &'a str,
    /// The last progress report (OSC 9;4 payload), or empty.
    pub osc_progress: &'a str,
}

impl<'a> DetectionInput<'a> {
    /// Just a screen, with no title or progress.
    #[must_use]
    pub fn screen(screen: &'a str) -> Self {
        Self {
            screen,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_alone_has_no_title_or_progress() {
        let input = DetectionInput::screen("❯ ");
        assert_eq!(
            (input.screen, input.osc_title, input.osc_progress),
            ("❯ ", "", "")
        );
    }
}
