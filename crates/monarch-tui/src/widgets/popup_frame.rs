//! The frame every pop-up is drawn in: its own raised surface, a rounded
//! border in the accent, the title in the accent and key hints dim, so a
//! pop-up stands apart from the panes behind it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Widget};

use super::Chrome;

/// A pop-up's frame, before it is drawn.
#[derive(Debug, Clone, Default)]
pub struct PopupFrame<'a> {
    chrome: Chrome<'a>,
    title: Option<Line<'a>>,
    footer: Option<Line<'a>>,
}

impl<'a> PopupFrame<'a> {
    /// A frame in `chrome`'s colours, untitled.
    #[must_use]
    pub fn new(chrome: Chrome<'a>) -> Self {
        Self {
            chrome,
            title: None,
            footer: None,
        }
    }

    /// The title on the top border, in the accent; spans keep their own
    /// styles over it.
    #[must_use]
    pub fn title(mut self, title: impl Into<Line<'a>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The key hints on the bottom border, dim.
    #[must_use]
    pub fn footer(mut self, footer: impl Into<Line<'a>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    /// What is inside a frame drawn over `area`: one cell in on each side.
    #[must_use]
    pub fn inner(area: Rect) -> Rect {
        Block::new().borders(Borders::ALL).inner(area)
    }

    /// Clears `area` of what was behind it, draws the frame there and
    /// returns its inside.
    pub fn draw(self, area: Rect, buf: &mut Buffer) -> Rect {
        let dim = Style::new().add_modifier(Modifier::DIM);
        let mut block = Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.chrome.popup_border())
            .style(self.chrome.popup());
        if let Some(title) = self.title {
            block = block.title(title.patch_style(self.chrome.highlight()));
        }
        if let Some(footer) = self.footer {
            block = block.title_bottom(footer.patch_style(dim));
        }
        Clear.render(area, buf);
        block.render(area, buf);
        Self::inner(area)
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::Theme;
    use ratatui::style::Color;

    use super::*;

    fn drawn(chrome: Chrome<'_>) -> (Buffer, Rect) {
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        buf.set_string(0, 0, "behind the pop-up...", Style::new().fg(Color::Red));
        let inner = PopupFrame::new(chrome)
            .title(" inbox ")
            .footer(" esc close ")
            .draw(area, &mut buf);
        (buf, inner)
    }

    #[test]
    fn a_frame_fills_its_surface_inside_a_rounded_accent_border() {
        let theme = Theme::dark();
        let (buf, inner) = drawn(Chrome::new(Some(&theme)));
        assert_eq!(inner, Rect::new(1, 1, 18, 2));
        assert_eq!(buf[(0, 0)].symbol(), "╭");
        assert_eq!(buf[(19, 3)].symbol(), "╯");
        assert_eq!(buf[(0, 0)].fg, Chrome::color(theme.accent));
        assert_eq!(buf[(5, 2)].bg, Chrome::color(theme.popup), "inside");
        assert_eq!(buf[(5, 2)].symbol(), " ", "what was behind is cleared");
        let title: String = (1..8).map(|x| buf[(x, 0)].symbol()).collect();
        assert_eq!(title, " inbox ");
        assert!(buf[(2, 0)].modifier.contains(Modifier::BOLD));
        assert!(buf[(2, 3)].modifier.contains(Modifier::DIM), "the footer");
    }

    #[test]
    fn without_colours_the_border_is_bold_on_the_hosts_background() {
        let (buf, _) = drawn(Chrome::default());
        assert!(buf[(0, 1)].modifier.contains(Modifier::BOLD));
        assert_eq!(buf[(5, 2)].bg, Color::Reset);
    }

    #[test]
    fn inside_matches_a_plain_border() {
        let area = Rect::new(3, 2, 10, 5);
        assert_eq!(PopupFrame::inner(area), Rect::new(4, 3, 8, 3));
    }
}
