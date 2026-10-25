//! Draws the welcome screen: a monarch beating its wings over the name,
//! the keys a newcomer needs, and where to go next.

mod butterfly;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use super::Chrome;
use butterfly::Part;

/// Widest the box gets.
const WIDTH: u16 = 78;
/// The name, spelled out a letter per tick.
const TITLE: &str = "M O N A R C H";
const TAGLINE: &str = "every agent in its own pane, and you always see which one needs you";
const SETUP: &str = "`monarch setup` lets it see your agents · `monarch doctor` checks it all";
const START: &str = "press any key to start";
/// Keys column plus gap, then the text.
const KEYS: usize = 13;

/// The welcome screen as drawn at one tick of its animation.
#[derive(Debug, Clone, Copy)]
pub struct WelcomeView<'a> {
    /// `(keys, what they do)`, e.g. `("ctrl+b %", "split …")`.
    pub keys: &'a [(String, String)],
    /// Animation ticks since it opened.
    pub tick: u32,
    /// Move; otherwise the open butterfly and the whole name, still.
    pub animate: bool,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl WelcomeView<'_> {
    /// Rows the butterfly takes, its rise included.
    const ART: u16 = butterfly::ROWS as u16 + 1;

    /// Key rows for `cols` columns.
    fn key_rows(&self, cols: usize) -> u16 {
        u16::try_from(self.keys.len().div_ceil(cols)).unwrap_or(u16::MAX)
    }

    /// Rows of everything but the butterfly: name, tagline, gap, keys,
    /// gap, two hints, borders.
    fn text_rows(&self, cols: usize) -> u16 {
        2 + 1 + self.key_rows(cols) + 1 + 2 + 2
    }

    fn style(&self, part: Part) -> Style {
        let bold = Style::new().add_modifier(Modifier::BOLD);
        match (self.chrome.theme, part) {
            (Some(t), Part::Wing) => Style::new().fg(Chrome::color(t.accent)),
            (Some(t), _) => bold.fg(Chrome::color(t.bar_text)),
            (None, Part::Wing) => Style::new(),
            (None, _) => bold,
        }
    }

    /// The butterfly, centred at the top of `area`.
    fn draw_art(&self, area: Rect, buf: &mut Buffer) {
        let (rows, lift) = butterfly::frame(if self.animate { self.tick } else { 0 });
        let width = u16::try_from(butterfly::WIDTH).unwrap_or(u16::MAX);
        let x = area.x + area.width.saturating_sub(width) / 2;
        let top = area.y + 1 - lift;
        for (y, row) in (top..area.bottom()).zip(&rows) {
            for (col, c) in row.chars().enumerate() {
                let cx = x + u16::try_from(col).unwrap_or(u16::MAX);
                if c != ' ' && cx < area.right() {
                    buf[(cx, y)]
                        .set_char(c)
                        .set_style(self.style(butterfly::part(c, col)));
                }
            }
        }
    }

    /// The name, a letter a tick, then the tagline.
    fn draw_title(&self, area: Rect, y: u16, buf: &mut Buffer) {
        let shown = if self.animate {
            usize::try_from(self.tick).map_or(usize::MAX, |t| t.saturating_mul(2))
        } else {
            TITLE.len()
        };
        let title: String = TITLE.chars().take(shown).collect();
        let title = format!("{title:<width$}", width = TITLE.len());
        centre(buf, area, y, &title, self.chrome.highlight());
        let dim = Style::new().add_modifier(Modifier::DIM);
        centre(buf, area, y + 1, TAGLINE, dim);
    }

    /// The keys in `cols` columns from row `y`, filled top to bottom.
    fn draw_keys(&self, area: Rect, y: u16, cols: usize, buf: &mut Buffer) {
        let rows = usize::from(self.key_rows(cols)).max(1);
        let col_width = usize::from(area.width.saturating_sub(4)) / cols;
        let bold = Style::new().add_modifier(Modifier::BOLD);
        for (i, (keys, text)) in self.keys.iter().enumerate() {
            let x = area.x + 2 + u16::try_from((i / rows) * col_width).unwrap_or(0);
            let row = y + u16::try_from(i % rows).unwrap_or(0);
            if row >= area.bottom() {
                continue;
            }
            buf.set_stringn(x, row, keys, KEYS - 1, bold);
            let text_x = x + u16::try_from(KEYS).unwrap_or(0);
            let room = col_width.saturating_sub(KEYS);
            buf.set_stringn(text_x, row, text, room, Style::new());
        }
    }
}

impl WelcomeView<'_> {
    /// The box in `area`, the butterfly's rows (none when short) and the
    /// key columns; `None` when there is no room at all.
    fn layout(&self, area: Rect) -> Option<(Rect, u16, usize)> {
        let width = WIDTH.min(area.width);
        let cols = if width >= 72 { 2 } else { 1 };
        let text = self.text_rows(cols);
        let art = if area.height >= text + Self::ART {
            Self::ART
        } else {
            0
        };
        let height = (text + art).min(area.height);
        if width < 30 || height < 5 {
            return None;
        }
        let x = area.x + (area.width - width) / 2;
        let y = area.y + (area.height - height) / 2;
        Some((Rect::new(x, y, width, height), art, cols))
    }
}

impl Widget for WelcomeView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Some((outer, art, cols)) = self.layout(area) else {
            return;
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().add_modifier(Modifier::DIM));
        let inner = block.inner(outer);
        Clear.render(outer, buf);
        block.render(outer, buf);
        if art > 0 {
            self.draw_art(
                Rect {
                    height: art,
                    ..inner
                },
                buf,
            );
        }
        let mut row = inner.y + art;
        self.draw_title(inner, row, buf);
        row += 3;
        self.draw_keys(inner, row, cols, buf);
        row += self.key_rows(cols) + 1;
        centre(
            buf,
            inner,
            row,
            SETUP,
            Style::new().add_modifier(Modifier::DIM),
        );
        centre(
            buf,
            inner,
            row + 1,
            START,
            Style::new().add_modifier(Modifier::BOLD),
        );
    }
}

/// `text` centred on row `y` of `area`, cut to fit.
fn centre(buf: &mut Buffer, area: Rect, y: u16, text: &str, style: Style) {
    if y >= area.bottom() {
        return;
    }
    let width = u16::try_from(Line::from(text).width()).unwrap_or(u16::MAX);
    let x = area.x + area.width.saturating_sub(width) / 2;
    buf.set_stringn(x, y, text, usize::from(area.width), style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Vec<(String, String)> {
        [
            ("ctrl+b %", "split the pane side by side"),
            ("ctrl+b a", "agents waiting for you"),
            ("ctrl+b ?", "every key"),
        ]
        .iter()
        .map(|(k, t)| ((*k).to_owned(), (*t).to_owned()))
        .collect()
    }

    fn draw(view: WelcomeView<'_>, w: u16, h: u16) -> Vec<String> {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        view.render(buf.area, &mut buf);
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    fn view(keys: &[(String, String)], tick: u32, animate: bool) -> WelcomeView<'_> {
        WelcomeView {
            keys,
            tick,
            animate,
            chrome: Chrome::default(),
        }
    }

    #[test]
    fn a_still_welcome_shows_everything() {
        let keys = keys();
        insta::assert_snapshot!(draw(view(&keys, 0, false), 80, 24).join("\n"));
    }

    #[test]
    fn the_name_spells_itself_out_and_the_wings_beat() {
        let keys = keys();
        let early = draw(view(&keys, 2, true), 80, 24).join("\n");
        assert!(
            early.contains("M O") && !early.contains("M O N A"),
            "{early}"
        );
        let folded = draw(view(&keys, 2, true), 80, 24);
        assert_ne!(folded, draw(view(&keys, 0, true), 80, 24));
        let short = draw(view(&keys, 99, true), 80, 12).join("\n");
        assert!(
            !short.contains('~') && short.contains("M O N A R C H"),
            "no room for art: {short}"
        );
        assert!(
            draw(view(&keys, 0, true), 20, 4)
                .iter()
                .all(String::is_empty),
            "too small"
        );
    }
}
