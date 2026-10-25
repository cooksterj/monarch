//! Draws the review pane over the panes: changed files on the left, the
//! selected file's diff on the right.

mod diff_column;

use monarch_core::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::ReviewState;

const KEYS: &str = " j/k file · e edit · space/b page · g/G ends · r refresh · esc close ";

/// The review pane as drawn.
#[derive(Debug, Clone, Copy)]
pub struct ReviewView<'a> {
    /// What to draw.
    pub state: &'a ReviewState,
    /// Colours for diff lines; `None` draws markers only (`NO_COLOR`).
    pub theme: Option<&'a Theme>,
}

impl ReviewView<'_> {
    /// How many diff lines fit when drawn `height` rows tall (for
    /// paging): less the borders and the file name.
    #[must_use]
    pub fn diff_rows(height: u16) -> usize {
        usize::from(height.saturating_sub(3))
    }

    fn title(&self) -> String {
        let Some(Some(review)) = self.state.review() else {
            return " review ".into();
        };
        let (added, removed) = review.totals();
        let branch = review.branch.as_deref().unwrap_or("detached");
        let files = review.files.len();
        let plural = if files == 1 { "" } else { "s" };
        format!(" review · {branch} · {files} file{plural} +{added} −{removed} ")
    }

    /// The message shown instead of files, if any.
    fn empty(&self) -> Option<&'static str> {
        match self.state.review() {
            None => Some("reading changes…"),
            Some(None) => Some("not in a git repository"),
            Some(Some(r)) if r.files.is_empty() => Some("no uncommitted changes"),
            Some(Some(_)) => None,
        }
    }

    fn draw_files(&self, area: Rect, buf: &mut Buffer) {
        let Some(Some(review)) = self.state.review() else {
            return;
        };
        let shown = usize::from(area.height);
        let skip = (self.state.file() + 1).saturating_sub(shown);
        for (i, file) in review.files.iter().enumerate().skip(skip).take(shown) {
            let y = area.y + u16::try_from(i - skip).unwrap_or(0);
            let row = Rect::new(area.x, y, area.width, 1);
            let style = if i == self.state.file() {
                Style::new().add_modifier(Modifier::REVERSED)
            } else {
                Style::new()
            };
            buf.set_style(row, style);
            let counts = format!("+{} −{}", file.added, file.removed);
            let counts_x = row
                .right()
                .saturating_sub(counts.chars().count() as u16 + 1);
            let label = format!(" {} {}", file.change.letter(), file.path);
            let room = usize::from(counts_x.saturating_sub(row.x + 1));
            buf.set_stringn(row.x, y, label, room, style);
            buf.set_stringn(
                counts_x,
                y,
                counts,
                usize::from(row.right() - counts_x),
                style.add_modifier(Modifier::DIM),
            );
        }
    }
}

impl Widget for ReviewView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 4 {
            return;
        }
        Clear.render(area, buf);
        let dim = Style::new().add_modifier(Modifier::DIM);
        let block = Block::new()
            .borders(Borders::ALL)
            .border_style(dim)
            .title(self.title())
            .title_bottom(Line::styled(KEYS, dim));
        let inner = block.inner(area);
        block.render(area, buf);
        if let Some(text) = self.empty() {
            buf.set_stringn(inner.x + 1, inner.y, text, usize::from(inner.width), dim);
            return;
        }
        let list_width = (inner.width / 3).clamp(16, 48).min(inner.width / 2);
        let files = Rect::new(inner.x, inner.y, list_width, inner.height);
        self.draw_files(files, buf);
        for y in inner.y..inner.bottom() {
            buf.set_string(inner.x + list_width, y, "│", dim);
        }
        let diff = Rect::new(
            inner.x + list_width + 2,
            inner.y,
            inner.width.saturating_sub(list_width + 2),
            inner.height,
        );
        if let Some(file) = self.state.selected() {
            diff_column::draw(file, self.state.scroll(), self.theme, diff, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::PaneId;
    use monarch_proto::{DiffLine, DiffLineKind, FileChange, Review, ReviewFile};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn draw(state: &ReviewState, width: u16, height: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        let view = ReviewView { state, theme: None };
        term.draw(|f| f.render_widget(view, f.area()))
            .expect("draw");
        let buf = term.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn loaded() -> ReviewState {
        let main = ReviewFile {
            added: 1,
            removed: 1,
            lines: vec![
                DiffLine::new(DiffLineKind::Hunk, "@@ -1,2 +1,2 @@"),
                DiffLine::new(DiffLineKind::Context, "fn main() {"),
                DiffLine::new(DiffLineKind::Removed, "\tprintln!(\"hi\");"),
                DiffLine::new(DiffLineKind::Added, "\tprintln!(\"hello\");"),
            ],
            ..ReviewFile::new("src/main.rs", FileChange::Modified)
        };
        let mut state = ReviewState::new(PaneId(1));
        state.load(Some(Review {
            root: "/w".into(),
            branch: Some("feat".into()),
            files: vec![main, ReviewFile::new("notes.md", FileChange::Untracked)],
            truncated: false,
        }));
        state
    }

    #[test]
    fn files_on_the_left_diff_on_the_right() {
        insta::assert_snapshot!(draw(&loaded(), 72, 9), @r#"
        ┌ review · feat · 2 files +1 −1 ───────────────────────────────────────┐
        │ M src/main.rs   +1 −1 │ src/main.rs                                  │
        │ ? notes.md      +0 −0 │ @@ -1,2 +1,2 @@                              │
        │                       │  fn main() {                                 │
        │                       │ -    println!("hi");                         │
        │                       │ +    println!("hello");                      │
        │                       │                                              │
        │                       │                                              │
        └ j/k file · e edit · space/b page · g/G ends · r refresh · esc close ─┘
        "#);
    }

    #[test]
    fn loading_no_repo_and_no_changes_say_so() {
        let mut state = ReviewState::new(PaneId(1));
        assert!(draw(&state, 40, 5).contains("reading changes…"));
        state.load(None);
        assert!(draw(&state, 40, 5).contains("not in a git repository"));
        state.load(Some(Review::default()));
        assert!(draw(&state, 40, 5).contains("no uncommitted changes"));
        assert_eq!(draw(&state, 10, 3).trim(), "", "too small to draw");
        assert_eq!(ReviewView::diff_rows(20), 17);
    }
}
