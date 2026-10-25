//! The review pane's state: the changes being shown, which file is
//! selected and how far its diff is scrolled. Pure data.

use monarch_core::PaneId;
use monarch_proto::{Review, ReviewFile};

use super::ReviewStep;
use crate::OverlayKey;

/// A review of one pane's checkout, loading until the server answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewState {
    pane: PaneId,
    /// `None` while loading; `Some(None)` outside a git repository.
    review: Option<Option<Review>>,
    file: usize,
    scroll: usize,
    page: usize,
}

impl ReviewState {
    /// A review of `pane`'s checkout, not yet loaded.
    #[must_use]
    pub fn new(pane: PaneId) -> Self {
        Self {
            pane,
            review: None,
            file: 0,
            scroll: 0,
            page: 10,
        }
    }

    /// The pane reviewed.
    #[must_use]
    pub fn pane(&self) -> PaneId {
        self.pane
    }

    /// The changes: `None` while loading, `Some(None)` outside git.
    #[must_use]
    pub fn review(&self) -> Option<Option<&Review>> {
        self.review.as_ref().map(Option::as_ref)
    }

    /// Index of the selected file.
    #[must_use]
    pub fn file(&self) -> usize {
        self.file
    }

    /// The selected file, once loaded.
    #[must_use]
    pub fn selected(&self) -> Option<&ReviewFile> {
        self.review.as_ref()?.as_ref()?.files.get(self.file)
    }

    /// The first diff line shown.
    #[must_use]
    pub fn scroll(&self) -> usize {
        self.scroll
    }

    /// Shows the server's answer, keeping the selected file (by path) and
    /// its scroll when it is still there.
    pub fn load(&mut self, review: Option<Review>) {
        let path = self.selected().map(|f| f.path.clone());
        let files = review.as_ref().map_or(&[][..], |r| r.files.as_slice());
        match files.iter().position(|f| Some(&f.path) == path.as_ref()) {
            Some(i) => self.file = i,
            None => (self.file, self.scroll) = (0, 0),
        }
        self.review = Some(review);
        self.scroll = self.scroll.min(self.max_scroll());
    }

    /// How many diff lines fit, for paging.
    pub fn set_page(&mut self, rows: usize) {
        self.page = rows.max(1);
    }

    /// Applies one key: `j`/`k` (arrows, Tab) pick a file; Space/PgDn and
    /// `b`/PgUp page the diff, `g`/`G` jump to its ends; `r` refreshes;
    /// Esc/`q` close.
    pub fn key(&mut self, key: OverlayKey) -> ReviewStep {
        match key {
            OverlayKey::Char('j') | OverlayKey::Down | OverlayKey::Tab => self.pick(1),
            OverlayKey::Char('k') | OverlayKey::Up | OverlayKey::BackTab => self.pick(-1),
            OverlayKey::Char(' ') | OverlayKey::PageDown | OverlayKey::Ctrl('d') => {
                self.scroll_by(self.page.cast_signed());
            }
            OverlayKey::Char('b') | OverlayKey::PageUp | OverlayKey::Ctrl('u') => {
                self.scroll_by(-self.page.cast_signed());
            }
            OverlayKey::Char('g') | OverlayKey::Home => self.scroll = 0,
            OverlayKey::Char('G') | OverlayKey::End => self.scroll = self.max_scroll(),
            OverlayKey::Char('r') => return ReviewStep::Refresh,
            OverlayKey::Esc | OverlayKey::Char('q') | OverlayKey::Ctrl('c' | 'g') => {
                return ReviewStep::Close;
            }
            _ => {}
        }
        ReviewStep::Open
    }

    fn files(&self) -> usize {
        self.review
            .as_ref()
            .and_then(Option::as_ref)
            .map_or(0, |r| r.files.len())
    }

    /// Selects the file `by` away, wrapping, with its diff at the top.
    fn pick(&mut self, by: isize) {
        let count = self.files();
        if count > 0 {
            self.file = (self.file.cast_signed() + by)
                .rem_euclid(count.cast_signed())
                .cast_unsigned();
            self.scroll = 0;
        }
    }

    fn scroll_by(&mut self, by: isize) {
        self.scroll = self.scroll.saturating_add_signed(by).min(self.max_scroll());
    }

    fn max_scroll(&self) -> usize {
        let lines = self.selected().map_or(0, |f| f.lines.len());
        lines.saturating_sub(self.page)
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::{DiffLine, DiffLineKind, FileChange};

    use super::*;

    fn review(paths: &[&str], lines: usize) -> Review {
        let file = |p: &&str| ReviewFile {
            lines: vec![DiffLine::new(DiffLineKind::Context, "x"); lines],
            ..ReviewFile::new(*p, FileChange::Modified)
        };
        Review {
            files: paths.iter().map(file).collect(),
            ..Review::default()
        }
    }

    #[test]
    fn loads_then_moves_between_files() {
        let mut state = ReviewState::new(PaneId(1));
        assert_eq!(state.review(), None, "loading");
        state.load(Some(review(&["a", "b", "c"], 3)));
        assert_eq!(state.selected().map(|f| f.path.as_str()), Some("a"));
        state.key(OverlayKey::Char('j'));
        state.key(OverlayKey::Down);
        assert_eq!(state.file(), 2);
        state.key(OverlayKey::Char('j'));
        assert_eq!(state.file(), 0, "wraps");
        state.key(OverlayKey::Up);
        assert_eq!(state.file(), 2, "wraps back");
        assert_eq!(state.key(OverlayKey::Char('r')), ReviewStep::Refresh);
        assert_eq!(state.key(OverlayKey::Char('q')), ReviewStep::Close);
    }

    #[test]
    fn pages_stay_within_the_diff() {
        let mut state = ReviewState::new(PaneId(1));
        state.load(Some(review(&["a"], 25)));
        state.set_page(10);
        state.key(OverlayKey::PageDown);
        assert_eq!(state.scroll(), 10);
        state.key(OverlayKey::Char(' '));
        state.key(OverlayKey::Char(' '));
        assert_eq!(state.scroll(), 15, "last page");
        state.key(OverlayKey::Char('b'));
        assert_eq!(state.scroll(), 5);
        state.key(OverlayKey::Char('G'));
        assert_eq!(state.scroll(), 15);
        state.key(OverlayKey::Char('g'));
        assert_eq!(state.scroll(), 0);
    }

    #[test]
    fn reloading_keeps_the_file_and_handles_no_repo() {
        let mut state = ReviewState::new(PaneId(1));
        state.load(Some(review(&["a", "b"], 30)));
        state.key(OverlayKey::Char('j'));
        state.key(OverlayKey::PageDown);
        state.load(Some(review(&["0", "a", "b"], 30)));
        assert_eq!(
            (state.file(), state.scroll()),
            (2, 10),
            "same file, same place"
        );
        state.load(Some(review(&["z"], 1)));
        assert_eq!((state.file(), state.scroll()), (0, 0));
        state.load(None);
        assert_eq!(state.review(), Some(None), "outside git");
        assert_eq!(
            state.key(OverlayKey::Char('j')),
            ReviewStep::Open,
            "nothing to pick"
        );
        assert_eq!(state.selected(), None);
    }
}
