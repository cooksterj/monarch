//! The divider lines between split panes.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;

use super::Chrome;

const UP: u8 = 1;
const DOWN: u8 = 2;
const LEFT: u8 = 4;
const RIGHT: u8 = 8;

/// Draws a box-drawing line in every cell of `area` that no pane covers,
/// joining lines into corners and tees. Lines touching the focused pane
/// are highlighted (bold, in the theme's accent), the rest dim.
#[derive(Debug, Clone, Copy)]
pub struct PaneBorders<'a> {
    /// Every visible pane's rectangle.
    pub panes: &'a [Rect],
    /// The focused pane's rectangle.
    pub focused: Option<Rect>,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl PaneBorders<'_> {
    fn is_divider(&self, area: Rect, x: i32, y: i32) -> bool {
        let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
            return false;
        };
        let inside = |r: &Rect| x >= r.x && x < r.right() && y >= r.y && y < r.bottom();
        inside(&area) && !self.panes.iter().any(inside)
    }

    fn touches_focus(&self, x: u16, y: u16) -> bool {
        self.focused.is_some_and(|r| {
            let near = |v: u16, lo: u16, hi: u16| v + 1 >= lo && v <= hi;
            near(x, r.x, r.right()) && near(y, r.y, r.bottom())
        })
    }
}

impl Widget for PaneBorders<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let (xi, yi) = (i32::from(x), i32::from(y));
                if !self.is_divider(area, xi, yi) {
                    continue;
                }
                let mask = [(0, -1, UP), (0, 1, DOWN), (-1, 0, LEFT), (1, 0, RIGHT)]
                    .iter()
                    .filter(|(dx, dy, _)| self.is_divider(area, xi + dx, yi + dy))
                    .fold(0, |m, (_, _, bit)| m | bit);
                let style = if self.touches_focus(x, y) {
                    self.chrome.highlight()
                } else {
                    Style::new().add_modifier(Modifier::DIM)
                };
                buf[(x, y)].set_symbol(glyph(mask)).set_style(style);
            }
        }
    }
}

/// The box-drawing character joining lines in the directions of `mask`.
fn glyph(mask: u8) -> &'static str {
    match mask {
        m if m == UP | DOWN | LEFT | RIGHT => "┼",
        m if m == UP | DOWN | RIGHT => "├",
        m if m == UP | DOWN | LEFT => "┤",
        m if m == DOWN | LEFT | RIGHT => "┬",
        m if m == UP | LEFT | RIGHT => "┴",
        m if m == DOWN | RIGHT => "┌",
        m if m == DOWN | LEFT => "┐",
        m if m == UP | RIGHT => "└",
        m if m == UP | LEFT => "┘",
        m if m & (LEFT | RIGHT) != 0 => "─",
        _ => "│",
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn draw(panes: &[Rect], focused: Option<Rect>, w: u16, h: u16) -> Buffer {
        let mut term = Terminal::new(TestBackend::new(w, h)).expect("terminal");
        term.draw(|f| {
            f.render_widget(
                PaneBorders {
                    panes,
                    focused,
                    chrome: Chrome::default(),
                },
                f.area(),
            );
        })
        .expect("draw");
        term.backend().buffer().clone()
    }

    fn rows(buf: &Buffer) -> Vec<String> {
        let area = buf.area;
        (0..area.height)
            .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    #[test]
    fn a_tee_joins_a_column_and_a_row_divider() {
        // A | (B over C) in a 7 × 5 window.
        let panes = [
            Rect::new(0, 0, 3, 5),
            Rect::new(4, 0, 3, 2),
            Rect::new(4, 3, 3, 2),
        ];
        let buf = draw(&panes, Some(panes[0]), 7, 5);
        assert_eq!(
            rows(&buf),
            ["   │   ", "   │   ", "   ├───", "   │   ", "   │   "]
        );
    }

    #[test]
    fn four_panes_meet_in_a_cross() {
        let panes = [
            Rect::new(0, 0, 2, 2),
            Rect::new(3, 0, 2, 2),
            Rect::new(0, 3, 2, 2),
            Rect::new(3, 3, 2, 2),
        ];
        assert_eq!(rows(&draw(&panes, None, 5, 5))[2], "──┼──");
    }

    #[test]
    fn lines_beside_the_focused_pane_are_bold() {
        let panes = [Rect::new(0, 0, 3, 3), Rect::new(4, 0, 3, 3)];
        let buf = draw(&panes, Some(panes[1]), 7, 3);
        assert!(buf[(3, 0)].modifier.contains(Modifier::BOLD));
        let unfocused = draw(&panes, None, 7, 3);
        assert!(unfocused[(3, 0)].modifier.contains(Modifier::DIM));
    }

    #[test]
    fn a_theme_colours_the_focused_lines() {
        let theme = monarch_core::Theme::dark();
        let panes = [Rect::new(0, 0, 3, 3), Rect::new(4, 0, 3, 3)];
        let mut buf = Buffer::empty(Rect::new(0, 0, 7, 3));
        PaneBorders {
            panes: &panes,
            focused: Some(panes[0]),
            chrome: Chrome::new(Some(&theme)),
        }
        .render(buf.area, &mut buf);
        assert_eq!(buf[(3, 1)].fg, Chrome::color(theme.accent));
    }

    #[test]
    fn a_single_pane_has_no_borders() {
        let panes = [Rect::new(0, 0, 4, 2)];
        assert!(
            rows(&draw(&panes, None, 4, 2))
                .iter()
                .all(|r| r.trim().is_empty())
        );
    }
}
