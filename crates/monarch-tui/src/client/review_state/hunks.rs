//! Moving the review by hunk, and which hunk a note is about: the one
//! holding the first diff line shown.

use monarch_proto::{DiffLine, DiffLineKind};

use super::ReviewState;
use crate::ReviewNote;

/// Most diff lines a note carries.
const MAX_NOTE_LINES: usize = 60;

impl ReviewState {
    /// Scrolls to the next (`forward`) or previous hunk header.
    pub(super) fn jump_hunk(&mut self, forward: bool) {
        let Some(file) = self.selected() else {
            return;
        };
        let headers = file
            .lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.kind == DiffLineKind::Hunk)
            .map(|(i, _)| i);
        let target = if forward {
            headers.filter(|i| *i > self.scroll).min()
        } else {
            headers.filter(|i| *i < self.scroll).max()
        };
        if let Some(line) = target {
            self.scroll = line.min(self.max_scroll());
        }
    }

    /// Scrolls one line down (`forward`) or up.
    pub(super) fn scroll_line(&mut self, forward: bool) {
        self.scroll_by(if forward { 1 } else { -1 });
    }

    /// What a note written now would be about: the selected file and the
    /// hunk holding the first line shown (the whole file when it has no
    /// hunks, as a new file); `None` with nothing to note.
    #[must_use]
    pub fn note_target(&self) -> Option<ReviewNote> {
        let file = self.selected()?;
        if file.lines.is_empty() {
            return None;
        }
        let is_hunk = |l: &DiffLine| l.kind == DiffLineKind::Hunk;
        let top = self.scroll.min(file.lines.len() - 1);
        let start = (0..=top)
            .rev()
            .find(|i| is_hunk(&file.lines[*i]))
            .unwrap_or(0);
        let end = (start + 1..file.lines.len())
            .find(|i| is_hunk(&file.lines[*i]))
            .unwrap_or(file.lines.len());
        Some(ReviewNote {
            pane: self.pane,
            path: file.path.clone(),
            lines: file.lines[start..end.min(start + MAX_NOTE_LINES)].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{FileChange, Review, ReviewFile};

    use super::*;
    use crate::{OverlayKey, ReviewStep};

    fn two_hunks() -> ReviewState {
        let mut lines = vec![DiffLine::new(DiffLineKind::Hunk, "@@ -1,2 +1,2 @@")];
        lines.extend((0..5).map(|i| DiffLine::new(DiffLineKind::Context, format!("a{i}"))));
        lines.push(DiffLine::new(DiffLineKind::Hunk, "@@ -20,2 +20,2 @@"));
        lines.extend((0..5).map(|i| DiffLine::new(DiffLineKind::Added, format!("b{i}"))));
        let file = ReviewFile {
            lines,
            ..ReviewFile::new("src/a.rs", FileChange::Modified)
        };
        let mut state = ReviewState::new(PaneId(2));
        state.load(Some(Review {
            files: vec![file, ReviewFile::new("empty.rs", FileChange::Untracked)],
            ..Review::default()
        }));
        state.set_page(4);
        state
    }

    #[test]
    fn hunk_keys_move_between_headers_and_lines_scroll_one() {
        let mut state = two_hunks();
        assert_eq!(state.key(OverlayKey::Char(']')), ReviewStep::Open);
        assert_eq!(state.scroll(), 6, "next header");
        state.key(OverlayKey::Char(']'));
        assert_eq!(state.scroll(), 6, "no further hunk: stays");
        state.key(OverlayKey::Ctrl('e'));
        assert_eq!(state.scroll(), 7);
        state.key(OverlayKey::Char('['));
        assert_eq!(state.scroll(), 6, "back to the hunk's header");
        state.key(OverlayKey::Char('['));
        state.key(OverlayKey::Ctrl('y'));
        assert_eq!(state.scroll(), 0, "top, and no further up");
    }

    #[test]
    fn the_note_is_on_the_hunk_holding_the_first_line_shown() {
        let mut state = two_hunks();
        let first = state.note_target().expect("a note");
        assert_eq!((first.pane, first.path.as_str()), (PaneId(2), "src/a.rs"));
        assert_eq!(first.hunk(), Some("@@ -1,2 +1,2 @@"));
        assert_eq!(first.lines.len(), 6, "up to the next header");
        state.key(OverlayKey::Ctrl('e'));
        state.key(OverlayKey::Ctrl('e'));
        assert_eq!(
            state
                .note_target()
                .and_then(|n| n.hunk().map(str::to_owned)),
            Some("@@ -1,2 +1,2 @@".into()),
            "still inside the first hunk"
        );
        state.key(OverlayKey::Char(']'));
        let second = state.note_target().expect("a note");
        assert_eq!(second.hunk(), Some("@@ -20,2 +20,2 @@"));
        assert_eq!(second.lines.len(), 6);
        assert_eq!(state.key(OverlayKey::Char('c')), ReviewStep::Note);
        state.key(OverlayKey::Char('j'));
        assert_eq!(state.note_target(), None, "nothing in an empty file");
    }
}
