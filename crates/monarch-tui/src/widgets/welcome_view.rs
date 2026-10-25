//! Draws the welcome screen: a swarm of monarchs rising past a card with
//! the name, the keys a newcomer needs, and where to go next.

mod swarm;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Widget};

use super::Chrome;
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

    /// How a butterfly at `depth` looks: faded toward the bar colour with
    /// distance, or dim, plain and bold without a theme.
    fn wing(&self, depth: Depth) -> Style {
        let near = depth == Depth::Near;
        let style = match self.chrome.theme {
            Some(t) => Style::new().fg(Chrome::color(t.accent.lerp(t.bar, depth.fade()))),
            None if depth == Depth::Far => Style::new().add_modifier(Modifier::DIM),
            None => Style::new(),
        };
        if near {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        }
    }

    /// Draws `flyers` into `area`, clipped to it.
    fn draw_flyers(&self, flyers: &[Flyer], area: Rect, buf: &mut Buffer) {
        for flyer in flyers {
            let style = self.wing(flyer.depth);
            for (dy, row) in (0..).zip(flyer.depth.sprite(flyer.open)) {
                for (dx, c) in (0..).zip(row.chars()) {
                    let (x, y) = (flyer.x + dx, flyer.y + dy);
                    if c == ' ' || x < 0 || y < 0 {
                        continue;
                    }
                    let x = area.x.saturating_add(u16::try_from(x).unwrap_or(u16::MAX));
                    let y = area.y.saturating_add(u16::try_from(y).unwrap_or(u16::MAX));
                    if x < area.right() && y < area.bottom() {
                        buf[(x, y)].set_char(c).set_style(style);
                    }
                }
            }
        }
    }
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
    /// Far and middle butterflies, then the card, then the near ones over
    /// it, so the card sits in the middle of the swarm.
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let tick = if self.animate { self.tick } else { STILL };
        let flyers = swarm::flyers(self.seed, tick, area.width, area.height);
        let near = flyers.partition_point(|f| f.depth < Depth::Near);
        self.draw_flyers(&flyers[..near], area, buf);
        if let Some((card, cols)) = self.card(area) {
            self.draw_card(card, cols, buf);
        }
        self.draw_flyers(&flyers[near..], area, buf);
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
    fn near_butterflies_fly_over_the_card() {
        let keys = keys();
        let v = view(&keys, 0, true);
        let area = Rect::new(0, 0, 80, 24);
        let (card, _) = v.card(area).expect("room");
        let over = (0..400).any(|tick| {
            swarm::flyers(v.seed, tick, 80, 24).iter().any(|f| {
                f.depth == Depth::Near
                    && f.y > i32::from(card.y) + 1
                    && f.y < i32::from(card.bottom()) - 2
                    && f.x > i32::from(card.x) + 2
                    && f.x < i32::from(card.right()) - 6
            })
        });
        assert!(over, "some near butterfly crosses the card");
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
