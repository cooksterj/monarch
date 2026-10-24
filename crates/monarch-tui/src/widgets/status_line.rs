//! The one-row bar that shows you are inside Monarch.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

/// Session, focused pane title and the key hint. Drawn in reverse video
/// so it follows the host's own colours until theme tokens land (M6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusLine<'a> {
    /// Session name.
    pub session: &'a str,
    /// The focused pane's title, if it set one.
    pub title: Option<&'a str>,
    /// The prefix as configured, e.g. `ctrl+b`.
    pub prefix: &'a str,
    /// The prefix was pressed and Monarch waits for a command key.
    pub pending: bool,
}

impl StatusLine<'_> {
    fn left(&self) -> String {
        match self.title.filter(|t| !t.is_empty()) {
            Some(title) => format!(" monarch │ {} │ {title} ", self.session),
            None => format!(" monarch │ {} ", self.session),
        }
    }

    fn right(&self) -> String {
        if self.pending {
            " PREFIX │ d detach ".to_owned()
        } else {
            format!(" {} d detach ", self.prefix)
        }
    }
}

impl Widget for StatusLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let base = Style::new().add_modifier(Modifier::REVERSED);
        let row = Rect { height: 1, ..area };
        buf.set_style(row, base);
        let right = self.right();
        let right_width = u16::try_from(right.chars().count())
            .unwrap_or(u16::MAX)
            .min(row.width);
        let left_width = usize::from(row.width - right_width);
        buf.set_stringn(
            row.x,
            row.y,
            self.left(),
            left_width,
            base.add_modifier(Modifier::BOLD),
        );
        let hint = if self.pending {
            base.add_modifier(Modifier::BOLD)
        } else {
            base
        };
        buf.set_stringn(
            row.right() - right_width,
            row.y,
            right,
            usize::from(right_width),
            hint,
        );
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn draw(line: StatusLine<'_>, width: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(width, 1)).expect("terminal");
        term.draw(|f| f.render_widget(line, f.area()))
            .expect("draw");
        let buf = term.backend().buffer();
        (0..width)
            .map(|x| buf[(x, 0)].symbol().to_owned())
            .collect()
    }

    fn line(title: Option<&str>, pending: bool) -> StatusLine<'_> {
        StatusLine {
            session: "main",
            title,
            prefix: "ctrl+b",
            pending,
        }
    }

    #[test]
    fn shows_session_title_and_hint() {
        insta::assert_snapshot!(draw(line(Some("vim notes.md"), false), 60), @" monarch │ main │ vim notes.md              ctrl+b d detach ");
    }

    #[test]
    fn pending_prefix_is_announced() {
        insta::assert_snapshot!(draw(line(None, true), 40), @" monarch │ main       PREFIX │ d detach ");
    }

    #[test]
    fn narrow_terminals_keep_the_hint() {
        insta::assert_snapshot!(draw(line(Some("long title"), false), 20), @" mo ctrl+b d detach ");
    }

    #[test]
    fn the_whole_row_is_reversed() {
        let mut term = Terminal::new(TestBackend::new(30, 1)).expect("terminal");
        term.draw(|f| f.render_widget(line(None, false), f.area()))
            .expect("draw");
        let cell = &term.backend().buffer()[(15, 0)];
        assert!(cell.modifier.contains(Modifier::REVERSED));
    }
}
