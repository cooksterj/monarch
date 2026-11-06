//! A review-note prompt: one line of text about a hunk, for the agent
//! whose change it is.

use super::{NoteStep, Picker, ReviewNote};
use crate::{OverlayKey, PickerStep};

const KEYS: &str = " enter send · esc cancel ";

/// The note being written and what it is about, in a picker without
/// rows. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePrompt {
    picker: Picker,
    note: ReviewNote,
}

impl NotePrompt {
    /// A prompt for a note on `note`'s hunk; `agent` names who gets it.
    #[must_use]
    pub fn new(note: ReviewNote, agent: &str) -> Self {
        let hint = format!("to {agent} in pane {} · the hunk goes with it", note.pane.0);
        let picker = Picker::new(note.title(), Vec::new()).with_text(KEYS, hint);
        Self { picker, note }
    }

    /// What the note is about.
    #[must_use]
    pub fn note(&self) -> &ReviewNote {
        &self.note
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Applies one key: type and edit the note, Enter sends it (blank
    /// cancels), Esc cancels.
    pub fn key(&mut self, key: OverlayKey) -> NoteStep {
        if key == OverlayKey::Enter {
            let text = self.picker.query().trim();
            return if text.is_empty() {
                NoteStep::Close
            } else {
                NoteStep::Send(text.to_owned())
            };
        }
        match self.picker.key(key) {
            PickerStep::Close => NoteStep::Close,
            _ => NoteStep::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{DiffLine, DiffLineKind};

    use super::*;

    fn prompt() -> NotePrompt {
        let note = ReviewNote {
            pane: PaneId(4),
            path: "src/a.rs".into(),
            lines: vec![DiffLine::new(DiffLineKind::Hunk, "@@ -1 +1 @@")],
        };
        NotePrompt::new(note, "claude")
    }

    fn typed(prompt: &mut NotePrompt, text: &str) {
        for c in text.chars() {
            assert_eq!(prompt.key(OverlayKey::Char(c)), NoteStep::Open);
        }
    }

    #[test]
    fn enter_sends_what_was_typed() {
        let mut prompt = prompt();
        assert_eq!(prompt.picker().title(), "note on src/a.rs @@ -1 +1 @@");
        assert_eq!(
            prompt.picker().text().map(|t| t.1),
            Some("to claude in pane 4 · the hunk goes with it")
        );
        typed(&mut prompt, " fix it ");
        assert_eq!(
            prompt.key(OverlayKey::Enter),
            NoteStep::Send("fix it".into())
        );
        assert_eq!(prompt.note().pane, PaneId(4));
    }

    #[test]
    fn blank_and_escape_cancel() {
        let mut prompt = prompt();
        assert_eq!(prompt.key(OverlayKey::Enter), NoteStep::Close, "blank");
        typed(&mut prompt, "x");
        assert_eq!(prompt.key(OverlayKey::Esc), NoteStep::Close);
    }
}
