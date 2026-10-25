//! Copy mode: a keyboard cursor over a pane's history (`prefix [`).
//!
//! Pure data: keys move a cursor addressed by absolute line, so it stays
//! on its text while output arrives; the caller sends the scroll, search
//! and copy requests the model asks for.

mod motion;
mod search;

use monarch_core::PaneId;
use monarch_proto::{GridPoint, Screen, SelectShape};

use super::CopyStep;
use crate::{OverlayKey, SelectionSpan};

/// A search being typed: its text and direction.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Prompt {
    text: String,
    backward: bool,
}

/// The cursor, the selection anchor, the view position asked of the
/// server, and search state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyMode {
    pane: PaneId,
    cursor: GridPoint,
    anchor: Option<(GridPoint, SelectShape)>,
    /// Top line last asked for, until the pane's screen shows it.
    requested: Option<i64>,
    scroll: Option<i64>,
    prompt: Option<Prompt>,
    search: Option<(String, bool)>,
}

impl CopyMode {
    /// Starts on `pane` with the cursor where the pane's cursor is.
    #[must_use]
    pub fn new(pane: PaneId, screen: &Screen) -> Self {
        let rows = screen.size.rows.saturating_sub(1);
        let (row, col) = if screen.cursor.visible {
            (screen.cursor.row.min(rows), screen.cursor.col)
        } else {
            (rows, 0)
        };
        Self {
            pane,
            cursor: GridPoint::at(screen, row, col),
            anchor: None,
            requested: None,
            scroll: None,
            prompt: None,
            search: None,
        }
    }

    /// The pane being browsed.
    #[must_use]
    pub fn pane(&self) -> PaneId {
        self.pane
    }

    /// Applies one key (see the module docs of the motions for the list).
    pub fn key(&mut self, key: OverlayKey, screen: &Screen) -> CopyStep {
        if self.prompt.is_some() {
            return self.prompt_key(key);
        }
        let step = match key {
            OverlayKey::Char('q') | OverlayKey::Ctrl('c') => CopyStep::Exit,
            OverlayKey::Esc if self.anchor.take().is_none() => CopyStep::Exit,
            OverlayKey::Char('v' | ' ') => self.toggle(SelectShape::Chars),
            OverlayKey::Char('V') => self.toggle(SelectShape::Lines),
            OverlayKey::Ctrl('v') => self.toggle(SelectShape::Block),
            OverlayKey::Char('y') | OverlayKey::Enter => self.copy(key),
            OverlayKey::Char(c @ ('/' | '?')) => {
                self.prompt = Some(Prompt {
                    text: String::new(),
                    backward: c == '/',
                });
                CopyStep::Stay
            }
            OverlayKey::Char('n') => self.repeat(false),
            OverlayKey::Char('N') => self.repeat(true),
            other => {
                self.motion(other, screen);
                CopyStep::Stay
            }
        };
        self.follow(screen);
        step
    }

    /// The line the view should scroll to show at its top, once per change.
    pub fn take_scroll(&mut self) -> Option<i64> {
        self.scroll.take()
    }

    /// The selection, if one is started: its two ends and shape.
    #[must_use]
    pub fn selection(&self) -> Option<(GridPoint, GridPoint, SelectShape)> {
        self.anchor.map(|(a, shape)| (a, self.cursor, shape))
    }

    /// The selection in `screen`'s viewport coordinates.
    #[must_use]
    pub fn span(&self, screen: &Screen) -> Option<SelectionSpan> {
        let (a, b, shape) = self.selection()?;
        Some(SelectionSpan::between(screen, a, b, shape))
    }

    /// The cursor's viewport cell (row, column), when it is in view.
    #[must_use]
    pub fn cursor_cell(&self, screen: &Screen) -> Option<(u16, u16)> {
        let row = u16::try_from(self.cursor.viewport_row(screen)).ok()?;
        (row < screen.size.rows).then_some((row, self.cursor.col))
    }

    /// What the status line says: the search being typed, or where the
    /// cursor is and what is being selected.
    #[must_use]
    pub fn status(&self, screen: &Screen) -> String {
        if let Some(p) = &self.prompt {
            let lead = if p.backward { '/' } else { '?' };
            return format!("{lead}{}_", p.text);
        }
        let total = i64::from(screen.history) + i64::from(screen.size.rows);
        let shape = match self.anchor.map(|(_, s)| s) {
            Some(SelectShape::Chars) => " · select",
            Some(SelectShape::Lines) => " · lines",
            Some(SelectShape::Block) => " · block",
            None => "",
        };
        format!("[copy] {}/{total}{shape}", self.cursor.line + 1)
    }

    /// Starts a selection of `shape`, switches to it, or (same shape)
    /// drops it.
    fn toggle(&mut self, shape: SelectShape) -> CopyStep {
        self.anchor = match self.anchor {
            Some((_, s)) if s == shape => None,
            Some((a, _)) => Some((a, shape)),
            None => Some((self.cursor, shape)),
        };
        CopyStep::Stay
    }

    /// `y` or Enter: copy the selection and leave; Enter with nothing
    /// selected just leaves.
    fn copy(&self, key: OverlayKey) -> CopyStep {
        match self.selection() {
            Some((start, end, shape)) => CopyStep::Copy { start, end, shape },
            None if key == OverlayKey::Enter => CopyStep::Exit,
            None => CopyStep::Stay,
        }
    }

    /// Keeps the cursor in view: asks for a scroll when it left the view.
    /// A request still on its way counts as where the view is.
    fn follow(&mut self, screen: &Screen) {
        let shown = GridPoint::top_line(screen);
        let base = match self.requested {
            Some(top) if top != shown => top,
            _ => shown,
        };
        let rows = i64::from(screen.size.rows.max(1));
        let history = i64::from(screen.history);
        let top = if self.cursor.line < base {
            self.cursor.line
        } else if self.cursor.line >= base + rows {
            self.cursor.line - rows + 1
        } else {
            base
        }
        .clamp(0, history);
        if top != base {
            self.scroll = Some(top);
            // At the bottom the view follows output, so it is never stale.
            self.requested = (top != history).then_some(top);
        }
    }
}

#[cfg(test)]
mod tests;
