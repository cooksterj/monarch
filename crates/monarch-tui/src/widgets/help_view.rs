//! Draws the help page over the panes: a box of headings and key lines,
//! with the search and position in its bottom border.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::{Chrome, PageView};
use crate::HelpState;

/// Widest the box gets.
const WIDTH: u16 = 84;

/// The help page as drawn.
#[derive(Debug, Clone, Copy)]
pub struct HelpView<'a> {
    /// What to draw.
    pub state: &'a HelpState,
    /// The prefix as configured, named in the title.
    pub prefix: &'a str,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl HelpView<'_> {
    /// Where the box goes inside `area`: full height, centred.
    #[must_use]
    pub fn placement(area: Rect) -> Rect {
        let width = WIDTH.min(area.width);
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y,
            width,
            area.height,
        )
    }

    /// Lines of help a box in `area` shows at once.
    #[must_use]
    pub fn body_rows(area: Rect) -> usize {
        PageView::body_rows(Self::placement(area))
    }

    fn footer(&self, place: &str) -> String {
        let filter = self.state.filter();
        if filter.is_empty() {
            format!("{place} type to search · ↑↓ PgUp PgDn scroll · esc close ")
        } else {
            format!(" search: {filter} ·{place} esc clears ")
        }
    }
}

impl Widget for HelpView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let outer = Self::placement(area);
        let mut page = PageView {
            title: format!(" help · keys follow {} ", self.prefix),
            footer: String::new(),
            lines: self.state.visible(),
            scroll: self.state.scroll(),
            empty: "no matches",
            chrome: self.chrome,
        };
        page.footer = self.footer(&page.place(PageView::body_rows(outer)));
        page.render(outer, buf);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::Keymap;

    use super::*;
    use crate::OverlayKey;

    fn draw(state: &HelpState, w: u16, h: u16) -> Vec<String> {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        HelpView {
            state,
            prefix: "ctrl+b",
            chrome: Chrome::default(),
        }
        .render(buf.area, &mut buf);
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .map(|s| s.trim_end().to_owned())
            .collect()
    }

    #[test]
    fn draws_headings_and_aligned_keys() {
        let mut state = HelpState::new(&Keymap::default());
        state.set_rows(8);
        insta::assert_snapshot!(draw(&state, 70, 10).join("\n"));
    }

    #[test]
    fn a_search_shows_in_the_border() {
        let mut state = HelpState::new(&Keymap::default());
        for c in "detach".chars() {
            state.key(OverlayKey::Char(c));
        }
        insta::assert_snapshot!(draw(&state, 70, 5).join("\n"));
        for c in "zzz".chars() {
            state.key(OverlayKey::Char(c));
        }
        assert!(draw(&state, 70, 5)[1].contains("no matches"));
        assert!(
            draw(&state, 10, 2).iter().all(|l| l.is_empty()),
            "too small"
        );
    }

    #[test]
    fn rows_leave_room_for_the_border() {
        assert_eq!(HelpView::body_rows(Rect::new(0, 0, 100, 30)), 28);
        assert_eq!(HelpView::placement(Rect::new(0, 0, 100, 30)).width, WIDTH);
    }
}
