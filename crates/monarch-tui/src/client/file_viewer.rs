//! The file viewer: one file from a checkout, highlighted, with a line
//! cursor, a line selection and vi-style keys to move, select, copy and
//! ask an agent about what is selected.

mod motion;
mod search;

use monarch_core::PaneId;
use monarch_proto::CodeFile;

use super::{FileViewerStep, Quote};
use crate::OverlayKey;

/// A file being read, where the cursor and view are, and the selection.
/// Lines count from 0 here and from 1 on screen. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileViewer {
    pane: PaneId,
    path: String,
    /// `None` until the server answers.
    file: Option<Result<CodeFile, String>>,
    cursor: usize,
    top: usize,
    /// Where a line selection started, if one is on.
    anchor: Option<usize>,
    /// Lines a page holds.
    page: usize,
    /// Digits typed before a motion (`42G`).
    count: Option<usize>,
    /// Columns scrolled off the left of every line.
    left: usize,
    /// A query being typed after `/`.
    typing: Option<String>,
    /// The last query searched for (`n`, `N`).
    query: Option<String>,
    /// What the last search found nothing for, to show.
    status: Option<String>,
}

impl FileViewer {
    /// A viewer for `path` from `pane`'s checkout, waiting for the file.
    #[must_use]
    pub fn new(pane: PaneId, path: impl Into<String>) -> Self {
        Self {
            pane,
            path: path.into(),
            file: None,
            cursor: 0,
            top: 0,
            anchor: None,
            page: 20,
            count: None,
            left: 0,
            typing: None,
            query: None,
            status: None,
        }
    }

    /// Shows the server's answer.
    pub fn load(&mut self, file: Result<CodeFile, String>) {
        self.file = Some(file);
    }

    /// The pane whose checkout the file is in.
    #[must_use]
    pub fn pane(&self) -> PaneId {
        self.pane
    }

    /// The file's path, from the checkout's top.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The file, or why it can't be shown; `None` while it loads.
    #[must_use]
    pub fn file(&self) -> Option<&Result<CodeFile, String>> {
        self.file.as_ref()
    }

    /// The cursor's line.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The first line shown.
    #[must_use]
    pub fn top(&self) -> usize {
        self.top
    }

    /// The selected lines, first and last, if a selection is on.
    #[must_use]
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// How many lines a page holds, as drawn.
    pub fn set_page(&mut self, rows: usize) {
        self.page = rows.max(1);
        self.follow();
    }

    /// One key: `j`/`k` and the arrows move, Ctrl-d/Ctrl-u half a page,
    /// Space/`b` (PgDn/PgUp) a page, `g`/`G` the ends (`42G` line 42),
    /// `h`/`l` scroll sideways (`0` back), `/` searches (`n`/`N` again),
    /// `V` or `v` selects lines, `a` asks an agent about them (or the
    /// cursor's line), `y` copies them, `e` opens the editor; Esc clears
    /// the selection, then closes, as `q` does.
    pub fn key(&mut self, key: OverlayKey) -> FileViewerStep {
        if self.search_key(key) {
            return FileViewerStep::Open;
        }
        if let OverlayKey::Char(c @ '0'..='9') = key
            && (c != '0' || self.count.is_some())
        {
            let digit = usize::from(c as u8 - b'0');
            self.count = Some(self.count.unwrap_or(0).saturating_mul(10) + digit);
            return FileViewerStep::Open;
        }
        let count = self.count.take();
        match key {
            OverlayKey::Char('q') | OverlayKey::Ctrl('c') => FileViewerStep::Close,
            OverlayKey::Esc if self.anchor.take().is_none() => FileViewerStep::Close,
            OverlayKey::Char('V' | 'v') => {
                self.anchor = match self.anchor {
                    Some(_) => None,
                    None => Some(self.cursor),
                };
                FileViewerStep::Open
            }
            OverlayKey::Char('a') => self
                .quote()
                .map_or(FileViewerStep::Open, FileViewerStep::Ask),
            OverlayKey::Char('y') => match self.quote() {
                Some(Quote::File { lines, .. }) => FileViewerStep::Copy(lines.join("\n")),
                _ => FileViewerStep::Open,
            },
            OverlayKey::Char('e') => FileViewerStep::Edit(self.path.clone()),
            key => {
                self.motion(key, count);
                FileViewerStep::Open
            }
        }
    }

    /// The selected lines (or the cursor's) as a quote for an agent.
    #[must_use]
    pub fn quote(&self) -> Option<Quote> {
        let Some(Ok(file)) = &self.file else {
            return None;
        };
        let (first, last) = self.selection().unwrap_or((self.cursor, self.cursor));
        let lines = file.text(first + 1, last + 1);
        (!lines.is_empty()).then(|| Quote::File {
            pane: self.pane,
            path: self.path.clone(),
            language: file.language.clone(),
            start: first + 1,
            lines,
        })
    }

    /// Columns scrolled off the left of every line.
    #[must_use]
    pub fn left(&self) -> usize {
        self.left
    }

    /// What the last search found nothing for.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Lines in the file; 0 while loading or on an error.
    fn len(&self) -> usize {
        match &self.file {
            Some(Ok(file)) => file.lines.len(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests;
