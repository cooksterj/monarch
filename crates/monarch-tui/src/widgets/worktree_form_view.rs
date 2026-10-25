//! Draws the new-worktree form: a small box near the top with the branch
//! being typed, what to start in it, and how the last attempt went.

use monarch_core::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use crate::{Chrome, WORKTREE_CHOICES, WorktreeForm};

/// Widest the box gets.
const WIDTH: u16 = 60;
/// Border, branch, start and border; messages take the rows below.
const CHROME: u16 = 4;
/// Most rows a message wraps onto.
const MESSAGE_ROWS: usize = 3;
/// Where the values start, after the labels.
const VALUE_X: u16 = 9;
const KEYS: &str = " tab choose · enter create · esc cancel ";

/// The form as drawn over the panes.
#[derive(Debug, Clone, Copy)]
pub struct WorktreeFormView<'a> {
    /// What to draw.
    pub form: &'a WorktreeForm,
    /// Colours, or `None` for no colour.
    pub theme: Option<&'a Theme>,
}

impl WorktreeFormView<'_> {
    /// Where the box goes inside `area`: centred, a little below the top,
    /// tall enough for the message.
    #[must_use]
    pub fn placement(&self, area: Rect) -> Rect {
        let width = WIDTH.min(area.width);
        let rows = self.message_lines(width.saturating_sub(4)).len();
        let height = (CHROME + u16::try_from(rows).unwrap_or(0)).min(area.height);
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 5,
            width,
            height,
        )
    }

    /// Where the cursor sits: after the branch name.
    #[must_use]
    pub fn cursor(&self, area: Rect) -> Position {
        let inner = inner(self.placement(area));
        let typed = u16::try_from(Line::from(self.form.name()).width()).unwrap_or(u16::MAX);
        let x = inner.x.saturating_add(VALUE_X).saturating_add(typed);
        Position::new(x.min(inner.right().saturating_sub(1)), inner.y)
    }

    fn draw_choices(&self, x: u16, y: u16, room: u16, buf: &mut Buffer) {
        let mut x = x;
        for (i, choice) in WORKTREE_CHOICES.iter().enumerate() {
            let name = choice.map_or("shell", |k| k.name());
            let style = if i == self.form.choice() {
                Style::new().add_modifier(Modifier::REVERSED)
            } else {
                Style::new()
            };
            let end = x.saturating_add(room).min(buf.area.right());
            let label = format!(" {name} ");
            x = buf
                .set_stringn(x, y, &label, usize::from(end.saturating_sub(x)), style)
                .0
                + 1;
        }
    }

    /// The message wrapped at spaces into lines of at most `width`
    /// characters (a longer word is cut).
    fn message_lines(&self, width: u16) -> Vec<String> {
        let Some((text, _)) = self.message() else {
            return Vec::new();
        };
        let width = usize::from(width.max(1));
        let mut lines: Vec<String> = Vec::new();
        for word in text.split(' ') {
            match lines.last_mut() {
                Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                    line.push(' ');
                    line.push_str(word);
                }
                _ => {
                    let chars: Vec<char> = word.chars().collect();
                    lines.extend(chars.chunks(width).map(|c| c.iter().collect::<String>()));
                }
            }
        }
        lines.truncate(MESSAGE_ROWS);
        lines
    }

    fn message(&self) -> Option<(&str, Style)> {
        let dim = Style::new().add_modifier(Modifier::DIM);
        if let Some(why) = self.form.error() {
            let red = self
                .theme
                .map_or(Style::new().add_modifier(Modifier::BOLD), |t| {
                    Style::new().fg(Chrome::color(t.error))
                });
            return Some((why, red));
        }
        self.form.busy().then_some(("creating the worktree…", dim))
    }
}

impl Widget for WorktreeFormView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let popup = self.placement(area);
        if popup.width < VALUE_X + 8 || popup.height < CHROME {
            return;
        }
        Clear.render(popup, buf);
        let dim = Style::new().add_modifier(Modifier::DIM);
        Block::new()
            .borders(Borders::ALL)
            .border_style(dim)
            .title(" new worktree ")
            .title_bottom(Line::styled(KEYS, dim))
            .render(popup, buf);
        let inner = inner(popup);
        let room = inner.width.saturating_sub(VALUE_X);
        let value_x = inner.x + VALUE_X;
        buf.set_string(inner.x + 1, inner.y, "branch", dim);
        buf.set_stringn(
            value_x,
            inner.y,
            self.form.name(),
            usize::from(room),
            Style::new(),
        );
        buf.set_string(inner.x + 1, inner.y + 1, "start", dim);
        self.draw_choices(value_x - 1, inner.y + 1, room + 1, buf);
        let style = self.message().map_or(dim, |(_, style)| style);
        let lines = self.message_lines(inner.width.saturating_sub(2));
        for (row, line) in (inner.y + 2..inner.bottom()).zip(lines) {
            buf.set_string(inner.x + 1, row, line, style);
        }
    }
}

fn inner(popup: Rect) -> Rect {
    Block::new().borders(Borders::ALL).inner(popup)
}

#[cfg(test)]
mod tests {
    use crate::OverlayKey;

    use super::*;

    fn draw(form: &WorktreeForm, width: u16) -> String {
        let area = Rect::new(0, 0, width, 8);
        let mut buf = Buffer::empty(area);
        WorktreeFormView { form, theme: None }.render(area, &mut buf);
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn branch_choices_and_errors() {
        let mut form = WorktreeForm::new("/r");
        for c in "feat/x".chars() {
            form.key(OverlayKey::Char(c));
        }
        form.key(OverlayKey::Tab);
        form.key(OverlayKey::Enter);
        form.failed("/Users/someone/Projects/repos/app-worktrees/feat-x already exists");
        insta::assert_snapshot!(draw(&form, 64), @"
        ┌ new worktree ────────────────────────────────────────────┐
        │ branch  feat/x                                           │
        │ start   shell   claude   codex                           │
        │ /Users/someone/Projects/repos/app-worktrees/feat-x       │
        │ already exists                                           │
        └ tab choose · enter create · esc cancel ──────────────────┘
        ");
        let view = WorktreeFormView {
            form: &form,
            theme: None,
        };
        assert_eq!(view.cursor(Rect::new(0, 0, 64, 6)), Position::new(18, 1));
    }

    #[test]
    fn a_fresh_form_has_no_message_row() {
        let form = WorktreeForm::new("/r");
        let view = WorktreeFormView {
            form: &form,
            theme: None,
        };
        assert_eq!(view.placement(Rect::new(0, 0, 80, 24)).height, CHROME);
    }

    #[test]
    fn too_small_draws_nothing() {
        let form = WorktreeForm::new("/r");
        assert_eq!(draw(&form, 12).trim(), "");
    }
}
