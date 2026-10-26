//! Draws the welcome screen: monarch butterflies rising past a card with
//! the name, the keys a newcomer needs, and where to go next.

mod sprite;
mod swarm;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use super::Chrome;
use sprite::Pixel;
use swarm::{Depth, Flyer};

/// Widest the card gets.
const WIDTH: u16 = 76;
/// The name, typed in two letters a tick.
const TITLE: &str = "M O N A R C H";
const TAGLINE: &str = "every agent in its own pane, and you always see which one needs you";
const SETUP: &str = "`monarch setup` lets it see your agents · `monarch doctor` checks it all";
const START: &str = "press any key to start";
/// Keys column plus gap, then the text.
const KEYS: usize = 13;
/// The tick a still welcome shows the swarm at.
const STILL: u32 = 0;

/// The welcome screen as drawn at one tick of its animation.
#[derive(Debug, Clone, Copy)]
pub struct WelcomeView<'a> {
    /// `(keys, what they do)`, e.g. `("ctrl+b %", "split …")`.
    pub keys: &'a [(String, String)],
    /// Animation ticks since it opened.
    pub tick: u32,
    /// Picks where the butterflies take off; each welcome gets its own.
    pub seed: u64,
    /// Move; otherwise the swarm holds still and the name is whole.
    pub animate: bool,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl WelcomeView<'_> {
    /// Key rows for `cols` columns.
    fn key_rows(&self, cols: usize) -> u16 {
        u16::try_from(self.keys.len().div_ceil(cols)).unwrap_or(u16::MAX)
    }

    /// The card's rows: borders, a gap, name, tagline, gap, keys, gap,
    /// two hints, a gap.
    fn card_rows(&self, cols: usize) -> u16 {
        2 + 1 + 2 + 1 + self.key_rows(cols) + 1 + 2 + 1
    }

    /// The card, centred in `area`, and its key columns; `None` when
    /// there is no room for it.
    fn card(&self, area: Rect) -> Option<(Rect, usize)> {
        let width = WIDTH.min(area.width.saturating_sub(4));
        let cols = if width >= 72 { 2 } else { 1 };
        let height = self.card_rows(cols).min(area.height);
        if width < 30 || height < 5 {
            return None;
        }
        let x = area.x + (area.width - width) / 2;
        let y = area.y + (area.height - height) / 2;
        Some((Rect::new(x, y, width, height), cols))
    }

    /// The colour of `pixel` at `depth`: the theme's wing colours, faded
    /// toward the bar with distance; without a theme, a silhouette.
    fn ink(&self, pixel: Pixel, depth: Depth) -> Option<Color> {
        let t = match (self.chrome.theme, pixel) {
            (_, Pixel::Clear) => return None,
            (None, _) => return Some(Color::Reset),
            (Some(t), _) => t,
        };
        let rgb = match pixel {
            Pixel::Wing => t.wing,
            Pixel::Spot => t.wing_spot,
            _ => t.wing_edge,
        };
        Some(Chrome::color(rgb.lerp(t.bar, depth.fade())))
    }

    /// Draws `flyers` into `area`, clipped to it, two pixels a cell; a
    /// cell's clear half keeps whatever is behind it.
    fn draw_flyers(&self, flyers: &[Flyer], area: Rect, buf: &mut Buffer) {
        let dim = self.chrome.theme.is_none();
        for f in flyers {
            let (wide, tall) = sprite::cells(f.depth);
            for (dx, dy) in (0..tall).flat_map(|dy| (0..wide).map(move |dx| (dx, dy))) {
                let ink =
                    |half| self.ink(sprite::pixel(f.depth, f.wings, dx, dy * 2 + half), f.depth);
                let Some(cell) = cell_at(area, f.x + dx, f.y + dy) else {
                    continue;
                };
                let (glyph, style) = match (ink(0), ink(1)) {
                    (None, None) => continue,
                    (Some(top), None) => ('▀', Style::new().fg(top)),
                    (None, Some(bottom)) => ('▄', Style::new().fg(bottom)),
                    (Some(top), Some(bottom)) if top == bottom => ('█', Style::new().fg(top)),
                    (Some(top), Some(bottom)) => ('▀', Style::new().fg(top).bg(bottom)),
                };
                let style = if dim && f.depth == Depth::Far {
                    style.add_modifier(Modifier::DIM)
                } else {
                    style
                };
                buf[cell].set_char(glyph).set_style(style);
            }
        }
    }
}

/// The cell `(x, y)` from `area`'s corner, if inside it.
fn cell_at(area: Rect, x: i32, y: i32) -> Option<(u16, u16)> {
    let x = area.x.checked_add(u16::try_from(x).ok()?)?;
    let y = area.y.checked_add(u16::try_from(y).ok()?)?;
    (x < area.right() && y < area.bottom()).then_some((x, y))
}

impl WelcomeView<'_> {
    /// The name, two letters a tick, then the tagline.
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

    /// The card's frame and everything in it.
    fn draw_card(&self, outer: Rect, cols: usize, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().add_modifier(Modifier::DIM));
        let inner = block.inner(outer);
        Clear.render(outer, buf);
        block.render(outer, buf);
        let mut row = inner.y + 1;
        self.draw_title(inner, row, buf);
        row += 3;
        self.draw_keys(inner, row, cols, buf);
        row += self.key_rows(cols) + 1;
        let dim = Style::new().add_modifier(Modifier::DIM);
        centre(buf, inner, row, SETUP, dim);
        let bold = Style::new().add_modifier(Modifier::BOLD);
        centre(buf, inner, row + 1, START, bold);
    }
}

impl Widget for WelcomeView<'_> {
    /// The butterflies, then the card over the far and middle ones; the
    /// near ones fly beside it.
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let tick = if self.animate { self.tick } else { STILL };
        let card = self.card(area);
        let span = card.map(|(c, _)| (c.x - area.x, c.right() - area.x));
        let flyers = swarm::flyers(self.seed, tick, area.width, area.height, span);
        self.draw_flyers(&flyers, area, buf);
        if let Some((card, cols)) = card {
            self.draw_card(card, cols, buf);
        }
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
            seed: 42,
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
    fn the_name_types_itself_in_and_the_swarm_flies() {
        let keys = keys();
        let early = draw(view(&keys, 2, true), 80, 24).join("\n");
        assert!(
            early.contains("M O") && !early.contains("M O N A"),
            "{early}"
        );
        assert_ne!(
            draw(view(&keys, 30, true), 80, 24),
            draw(view(&keys, 31, true), 80, 24),
            "it moves"
        );
        let still = draw(view(&keys, 30, false), 80, 24);
        assert_eq!(still, draw(view(&keys, 31, false), 80, 24), "or holds");
        assert!(still.join("\n").contains("press any key"));
    }

    #[test]
    fn butterflies_are_drawn_in_wing_colours_and_never_on_the_card() {
        let keys = keys();
        let theme = monarch_core::Theme::dark();
        let v = WelcomeView {
            chrome: Chrome::new(Some(&theme)),
            ..view(&keys, 0, true)
        };
        let area = Rect::new(0, 0, 160, 40);
        let (card, _) = v.card(area).expect("room");
        let wing = Chrome::color(theme.wing);
        let seen = (0..60).any(|tick| {
            let mut buf = Buffer::empty(area);
            WelcomeView { tick, ..v }.render(area, &mut buf);
            let inside = |x, y| card.contains((x, y).into());
            for (x, y) in (0..40).flat_map(|y| (0..160).map(move |x| (x, y))) {
                let c = &buf[(x, y)];
                assert!(!(inside(x, y) && "▀▄█".contains(c.symbol())), "{x},{y}");
            }
            buf.content.iter().any(|c| c.fg == wing || c.bg == wing)
        });
        assert!(seen, "a near monarch in full colour");
    }

    #[test]
    fn small_hosts_draw_what_fits() {
        let keys = keys();
        for (w, h) in [(20, 4), (1, 1), (34, 8), (200, 60)] {
            let rows = draw(view(&keys, 99, true), w, h);
            assert_eq!(rows.len(), usize::from(h));
        }
        assert!(
            !draw(view(&keys, 0, false), 20, 4)
                .join("")
                .contains("press"),
            "no card when too small"
        );
    }
}
