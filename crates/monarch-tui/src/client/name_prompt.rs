//! A rename prompt: one line of text naming a tab or a pane.

use super::{NameStep, NameTarget, Picker};
use crate::{OverlayKey, PickerStep};

const KEYS: &str = " enter rename · blank + enter: automatic name · esc cancel ";

/// What is being named and the name typed so far, in a picker without
/// rows. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamePrompt {
    picker: Picker,
    target: NameTarget,
}

impl NamePrompt {
    /// A prompt titled `title` for `target`, starting from the name the
    /// user gave it (`given`); `shown` is what it is called now.
    #[must_use]
    pub fn new(target: NameTarget, title: String, given: Option<&str>, shown: &str) -> Self {
        let picker = Picker::new(title, Vec::new())
            .with_text(KEYS, format!("now: {shown}"))
            .with_query(given.unwrap_or_default());
        Self { picker, target }
    }

    /// What is being named.
    #[must_use]
    pub fn target(&self) -> NameTarget {
        self.target
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Applies one key: type and edit the name, Enter renames, Esc
    /// cancels.
    pub fn key(&mut self, key: OverlayKey) -> NameStep {
        if key == OverlayKey::Enter {
            return NameStep::Rename(self.picker.query().trim().to_owned());
        }
        match self.picker.key(key) {
            PickerStep::Close => NameStep::Close,
            _ => NameStep::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;

    use super::*;

    fn prompt(given: Option<&str>) -> NamePrompt {
        NamePrompt::new(
            NameTarget::Pane(PaneId(4)),
            "rename pane".into(),
            given,
            "claude",
        )
    }

    fn typed(prompt: &mut NamePrompt, text: &str) {
        for c in text.chars() {
            assert_eq!(prompt.key(OverlayKey::Char(c)), NameStep::Open);
        }
    }

    #[test]
    fn enter_renames_with_what_was_typed() {
        let mut prompt = prompt(None);
        assert_eq!(prompt.picker().title(), "rename pane");
        assert_eq!(prompt.picker().text().map(|t| t.1), Some("now: claude"));
        typed(&mut prompt, " api ");
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            NameStep::Rename("api".into())
        );
        assert_eq!(prompt.target(), NameTarget::Pane(PaneId(4)));
    }

    #[test]
    fn starts_from_the_given_name_and_can_be_cleared() {
        let mut prompt = prompt(Some("web"));
        assert_eq!(prompt.picker().query(), "web");
        prompt.key(OverlayKey::Ctrl('u'));
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            NameStep::Rename(String::new())
        );
    }

    #[test]
    fn escape_cancels() {
        let mut prompt = prompt(None);
        typed(&mut prompt, "x");
        assert_eq!(prompt.key(OverlayKey::Esc), NameStep::Close);
    }
}
