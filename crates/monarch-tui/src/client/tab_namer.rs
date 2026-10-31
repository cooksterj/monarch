//! The rename-tab prompt: one line of text naming the shown tab.

use monarch_core::PaneId;

use super::{Picker, TabNameStep};
use crate::OverlayKey;

const KEYS: &str = " enter rename · blank + enter: automatic name · esc cancel ";

/// The tab being named (by a pane in it) and the name typed so far, in a
/// picker without rows. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabNamer {
    picker: Picker,
    pane: PaneId,
}

impl TabNamer {
    /// A prompt for tab number `number` holding `pane`, starting from the
    /// name the user gave it (`given`); `shown` is what it is called now.
    #[must_use]
    pub fn new(pane: PaneId, number: usize, given: Option<&str>, shown: &str) -> Self {
        let empty = format!("now: {shown}");
        let picker = Picker::new(format!("rename tab {number}"), Vec::new())
            .with_text(KEYS, empty)
            .with_query(given.unwrap_or_default());
        Self { picker, pane }
    }

    /// A pane in the tab being named.
    #[must_use]
    pub fn pane(&self) -> PaneId {
        self.pane
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Applies one key: type and edit the name, Enter renames, Esc
    /// cancels.
    pub fn key(&mut self, key: OverlayKey) -> TabNameStep {
        if key == OverlayKey::Enter {
            return TabNameStep::Rename(self.picker.query().trim().to_owned());
        }
        match self.picker.key(key) {
            crate::PickerStep::Close => TabNameStep::Close,
            _ => TabNameStep::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(namer: &mut TabNamer, text: &str) {
        for c in text.chars() {
            assert_eq!(namer.key(OverlayKey::Char(c)), TabNameStep::Open);
        }
    }

    #[test]
    fn enter_renames_with_what_was_typed() {
        let mut namer = TabNamer::new(PaneId(4), 2, None, "claude");
        assert_eq!(namer.picker().title(), "rename tab 2");
        assert_eq!(namer.picker().text().map(|t| t.1), Some("now: claude"));
        typed(&mut namer, " api ");
        assert_eq!(
            namer.key(OverlayKey::Enter),
            TabNameStep::Rename("api".into())
        );
        assert_eq!(namer.pane(), PaneId(4));
    }

    #[test]
    fn starts_from_the_given_name_and_can_be_cleared() {
        let mut namer = TabNamer::new(PaneId(1), 1, Some("web"), "web");
        assert_eq!(namer.picker().query(), "web");
        namer.key(OverlayKey::Ctrl('u'));
        assert_eq!(
            namer.key(OverlayKey::Enter),
            TabNameStep::Rename(String::new())
        );
    }

    #[test]
    fn escape_cancels() {
        let mut namer = TabNamer::new(PaneId(1), 1, None, "zsh");
        typed(&mut namer, "x");
        assert_eq!(namer.key(OverlayKey::Esc), TabNameStep::Close);
    }
}
