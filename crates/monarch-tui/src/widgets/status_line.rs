//! The one-row bar that shows you are inside Monarch: session, tabs, hint.

use monarch_core::Key;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use super::{Chrome, MouseOwner};

/// Session, tab bar and the key hint, on the theme's bar colour with the
/// shown tab and a waiting prefix in its accent. Without a theme the bar
/// is reverse video and the shown tab the one run that is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusLine<'a> {
    /// Session name.
    pub session: &'a str,
    /// One label per tab (`1 vim`).
    pub tabs: &'a [String],
    /// Index of the shown tab in `tabs`.
    pub active_tab: usize,
    /// The prefix as configured, e.g. `ctrl+b`.
    pub prefix: &'a str,
    /// The key bound to detach, if any, for the hint.
    pub detach: Option<Key>,
    /// The prefix was pressed and Monarch waits for a command key.
    pub pending: bool,
    /// A short notice (e.g. "copied 12 characters") shown instead of the
    /// hint for a moment.
    pub message: Option<&'a str>,
    /// Who has the mouse, always shown (room permitting) so clicks that
    /// do nothing in Monarch make sense.
    pub mouse: MouseOwner,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl StatusLine<'_> {
    /// The mouse badge, then the notice or the key hint. The badge is
    /// dropped when it would push the session name off a narrow row.
    fn right(&self, width: u16) -> String {
        let hint = self.hint();
        let badge = self.mouse.badge();
        let head = format!(" monarch │ {} │", self.session);
        let wanted = [head.as_str(), badge, hint.as_str()]
            .iter()
            .map(|s| s.chars().count())
            .sum::<usize>();
        if wanted <= usize::from(width) {
            format!("{badge}{hint}")
        } else {
            hint
        }
    }

    fn hint(&self) -> String {
        if !self.pending
            && let Some(message) = self.message.filter(|m| !m.is_empty())
        {
            return format!(" {message} ");
        }
        let lead = if self.pending {
            "PREFIX │"
        } else {
            self.prefix
        };
        match self.detach {
            Some(key) => format!(" {lead} {key} detach "),
            None => format!(" {lead} "),
        }
    }

    /// Session name then tabs, from the row's start up to `end`.
    fn draw_left(&self, row: Rect, end: u16, buf: &mut Buffer) {
        let base = self.chrome.bar();
        let mut x = row.x;
        let mut put = |text: &str, style: Style| {
            let room = usize::from(end.saturating_sub(x));
            x = buf.set_stringn(x, row.y, text, room, style).0;
        };
        put(
            &format!(" monarch │ {} │", self.session),
            base.add_modifier(Modifier::BOLD),
        );
        for (i, label) in self.tabs.iter().enumerate() {
            let style = if i == self.active_tab {
                self.chrome.accent()
            } else {
                base
            };
            put(" ", base);
            put(&format!(" {label} "), style);
        }
    }
}

impl Widget for StatusLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let base = self.chrome.bar();
        let row = Rect { height: 1, ..area };
        buf.set_style(row, base);
        let right = self.right(row.width);
        let right_width = u16::try_from(right.chars().count())
            .unwrap_or(u16::MAX)
            .min(row.width);
        let right_x = row.right() - right_width;
        self.draw_left(row, right_x, buf);
        let hint = if self.pending {
            self.chrome.accent()
        } else {
            base
        };
        buf.set_stringn(right_x, row.y, right, usize::from(right_width), hint);
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

    fn line(tabs: &[String], pending: bool) -> StatusLine<'_> {
        StatusLine {
            session: "main",
            tabs,
            active_tab: 0,
            prefix: "ctrl+b",
            detach: Some(Key::Char('d')),
            pending,
            message: None,
            mouse: MouseOwner::Monarch,
            chrome: Chrome::default(),
        }
    }

    fn tabs() -> Vec<String> {
        vec!["1 vim".into(), "2 zsh".into()]
    }

    #[test]
    fn shows_session_tabs_and_hint() {
        let tabs = tabs();
        insta::assert_snapshot!(draw(line(&tabs, false), 80), @" monarch │ main │  1 vim   2 zsh               MOUSE: MONARCH │ ctrl+b d detach ");
    }

    #[test]
    fn the_shown_tab_is_not_reversed() {
        let tabs = tabs();
        let mut term = Terminal::new(TestBackend::new(80, 1)).expect("terminal");
        let second = StatusLine {
            active_tab: 1,
            ..line(&tabs, false)
        };
        term.draw(|f| f.render_widget(second, f.area()))
            .expect("draw");
        let buf = term.backend().buffer();
        assert!(buf[(20, 0)].modifier.contains(Modifier::REVERSED), "tab 1");
        assert!(!buf[(28, 0)].modifier.contains(Modifier::REVERSED), "tab 2");
    }

    #[test]
    fn pending_prefix_is_announced() {
        insta::assert_snapshot!(draw(line(&[], true), 40), @" monarch │ main │     PREFIX │ d detach ");
    }

    #[test]
    fn the_hint_follows_the_keymap() {
        let herdr = StatusLine {
            detach: Some(Key::Char('q')),
            ..line(&[], false)
        };
        insta::assert_snapshot!(draw(herdr, 40), @" monarch │ main │       ctrl+b q detach ");
        let unbound = StatusLine {
            detach: None,
            ..line(&[], true)
        };
        insta::assert_snapshot!(draw(unbound, 30), @" monarch │ main │    PREFIX │ ");
    }

    #[test]
    fn narrow_terminals_keep_the_hint() {
        let tabs = tabs();
        insta::assert_snapshot!(draw(line(&tabs, false), 20), @" mo ctrl+b d detach ");
    }

    #[test]
    fn a_message_replaces_the_hint_until_the_prefix_is_pressed() {
        let tabs = tabs();
        let with = StatusLine {
            message: Some("copied 5 characters"),
            ..line(&tabs, false)
        };
        insta::assert_snapshot!(draw(with, 80), @" monarch │ main │  1 vim   2 zsh           MOUSE: MONARCH │ copied 5 characters ");
        let pending = StatusLine {
            pending: true,
            ..with
        };
        assert!(draw(pending, 60).contains("PREFIX"));
    }

    #[test]
    fn the_mouse_badge_stays_beside_notices_and_the_prefix() {
        let tabs = tabs();
        let mouse = StatusLine {
            mouse: MouseOwner::Terminal,
            ..line(&tabs, false)
        };
        insta::assert_snapshot!(draw(mouse, 60), @" monarch │ main │  1 vim  MOUSE: TERMINAL │ ctrl+b d detach ");
        let noticed = StatusLine {
            message: Some("mouse: terminal"),
            ..mouse
        };
        assert!(draw(noticed, 60).contains("MOUSE: TERMINAL │ mouse: terminal"));
        let pending = StatusLine {
            pending: true,
            ..mouse
        };
        assert!(draw(pending, 60).contains("MOUSE: TERMINAL │ PREFIX │"));
    }

    #[test]
    fn the_whole_row_is_reversed() {
        let mut term = Terminal::new(TestBackend::new(30, 1)).expect("terminal");
        term.draw(|f| f.render_widget(line(&[], false), f.area()))
            .expect("draw");
        let cell = &term.backend().buffer()[(15, 0)];
        assert!(cell.modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn a_theme_colours_the_bar_and_the_shown_tab() {
        let theme = monarch_core::Theme::light();
        let tabs = tabs();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        let themed = StatusLine {
            active_tab: 1,
            chrome: Chrome::new(Some(&theme)),
            ..line(&tabs, true)
        };
        themed.render(buf.area, &mut buf);
        let bar = Chrome::color(theme.bar);
        assert_eq!(buf[(20, 0)].bg, bar, "tab 1");
        assert_eq!(buf[(28, 0)].bg, Chrome::color(theme.accent), "tab 2");
        assert_eq!(buf[(79, 0)].bg, Chrome::color(theme.accent), "PREFIX");
        assert!(!buf[(20, 0)].modifier.contains(Modifier::REVERSED));
    }
}
