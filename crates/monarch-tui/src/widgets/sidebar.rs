//! Draws the sidebar: panes grouped under their project and branch, the
//! focused one marked, with a divider on its right edge.

use monarch_core::PaneId;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use super::{SidebarGroup, SidebarRow};

/// What one sidebar line shows.
enum Line<'a> {
    Heading(&'a SidebarGroup),
    Pane(&'a SidebarRow),
    Gap,
}

/// The sidebar for an area whose last column is the divider.
#[derive(Debug, Clone, Copy)]
pub struct Sidebar<'a> {
    /// The groups, in order.
    pub groups: &'a [SidebarGroup],
}

impl<'a> Sidebar<'a> {
    /// Each pane row's cells in `area`, for clicks to focus it.
    #[must_use]
    pub fn targets(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let width = area.width.saturating_sub(1);
        self.lines(area)
            .filter_map(|(y, line)| match line {
                Line::Pane(row) => Some((row.pane, Rect::new(area.x, y, width, 1))),
                Line::Heading(_) | Line::Gap => None,
            })
            .collect()
    }

    /// Every line that fits in `area`, with its row.
    fn lines(&self, area: Rect) -> impl Iterator<Item = (u16, Line<'a>)> + 'a {
        let lines = self.groups.iter().enumerate().flat_map(|(i, group)| {
            let gap = (i > 0).then_some(Line::Gap);
            gap.into_iter()
                .chain(std::iter::once(Line::Heading(group)))
                .chain(group.rows.iter().map(Line::Pane))
        });
        (area.y..area.bottom()).zip(lines)
    }
}

impl Widget for Sidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 {
            return;
        }
        let width = area.width - 1;
        let dim = Style::new().add_modifier(Modifier::DIM);
        for y in area.y..area.bottom() {
            buf.set_string(area.x + width, y, "│", dim);
        }
        for (y, line) in self.lines(area) {
            let row = Rect::new(area.x, y, width, 1);
            match line {
                Line::Heading(group) => heading(group, row, buf),
                Line::Pane(pane) => pane_line(pane, row, buf),
                Line::Gap => {}
            }
        }
    }
}

/// `▾ project  branch`, the branch dim and shortened so the checkout's
/// badge (`±3 ↑2`) fits at the right.
fn heading(group: &SidebarGroup, row: Rect, buf: &mut Buffer) {
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let title = format!("▾ {}", group.title);
    let (x, _) = buf.set_stringn(row.x, row.y, &title, usize::from(row.width), bold);
    let badge = group.status.map(|s| s.to_string()).unwrap_or_default();
    let badge_width = u16::try_from(Span::raw(badge.as_str()).width()).unwrap_or(u16::MAX);
    let left = row.right().saturating_sub(x);
    let room = left
        .checked_sub(badge_width + 1)
        .filter(|_| !badge.is_empty());
    let detail_room = room.unwrap_or(left);
    if let Some(detail) = group.detail.as_deref().filter(|_| detail_room > 2) {
        let dim = Style::new().add_modifier(Modifier::DIM);
        let detail = shorten(detail, usize::from(detail_room - 2));
        buf.set_string(x + 2, row.y, detail, dim);
    }
    if room.is_some() {
        let x = row.right() - badge_width;
        buf.set_string(x, row.y, &badge, Style::new());
    }
}

/// `text` cut to `max` characters, ending in `…` when cut.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// `▸ ● label  detail`: the focus marker, the state's glyph and the
/// pane's activity or directory, dim.
fn pane_line(pane: &SidebarRow, row: Rect, buf: &mut Buffer) {
    let marker = if pane.focused { "▸" } else { " " };
    let glyph = pane.look.map_or(pane.state.glyph(), |l| l.symbol);
    let text = format!("{marker} {glyph} {}", pane.label);
    let width = usize::from(row.width);
    let (x, _) = buf.set_stringn(row.x, row.y, &text, width, row_style(pane));
    if row.width > 2 {
        paint_glyph(pane, &mut buf[(row.x + 2, row.y)]);
    }
    let left = usize::from(row.right() - x);
    if let Some(detail) = pane.detail.as_deref().filter(|_| left > 2) {
        let dim = Style::new().add_modifier(Modifier::DIM);
        buf.set_stringn(x + 2, row.y, detail, left - 2, dim);
    }
}

/// Colours the glyph cell from its look; without a colour, states that
/// need attention stand out in bold reverse video instead.
fn paint_glyph(row: &SidebarRow, cell: &mut Cell) {
    match row.look.and_then(|l| Some((l.color?, l.bold))) {
        Some((c, bold)) => {
            cell.set_fg(Color::Rgb(c.r, c.g, c.b));
            if bold {
                cell.modifier.insert(Modifier::BOLD);
            }
        }
        None if row.state.needs_attention() => {
            cell.modifier.insert(Modifier::BOLD | Modifier::REVERSED);
        }
        None => {}
    }
}

fn row_style(row: &SidebarRow) -> Style {
    match (row.focused, row.shown) {
        (true, _) => Style::new().add_modifier(Modifier::BOLD),
        (false, true) => Style::new(),
        (false, false) => Style::new().add_modifier(Modifier::DIM),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use monarch_core::AgentState;
    use monarch_proto::CheckoutStatus;

    use super::*;
    use crate::GlyphLook;

    fn row(pane: u32, label: &str, detail: Option<&str>) -> SidebarRow {
        SidebarRow {
            pane: PaneId(pane),
            label: label.into(),
            state: AgentState::Unknown,
            look: None,
            detail: detail.map(Into::into),
            focused: pane == 1,
            shown: pane < 3,
        }
    }

    fn groups() -> Vec<SidebarGroup> {
        vec![
            SidebarGroup {
                worktree: Some("/w/monarch".into()),
                title: "monarch".into(),
                detail: Some("feat/sidebar".into()),
                status: None,
                rows: vec![row(1, "1 claude", None), row(2, "1 zsh", Some("src"))],
            },
            SidebarGroup {
                rows: vec![row(3, "2 htop", Some("tmp"))],
                ..SidebarGroup::scratch()
            },
        ]
    }

    fn draw(groups: &[SidebarGroup], width: u16, height: u16) -> (String, Buffer) {
        let mut term = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        term.draw(|f| f.render_widget(Sidebar { groups }, f.area()))
            .expect("draw");
        let buf = term.backend().buffer().clone();
        let text = (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        (text, buf)
    }

    #[test]
    fn groups_panes_under_project_and_branch() {
        let (text, buf) = draw(&groups(), 24, 7);
        insta::assert_snapshot!(text, @r"
        ▾ monarch  feat/sidebar│
        ▸ $ 1 claude           │
          $ 1 zsh  src         │
                               │
        ▾ Scratch              │
          $ 2 htop  tmp        │
                               │
        ");
        assert!(buf[(2, 1)].modifier.contains(Modifier::BOLD), "focused");
        assert!(buf[(4, 5)].modifier.contains(Modifier::DIM), "hidden tab");
        assert!(!buf[(4, 2)].modifier.contains(Modifier::DIM), "shown tab");
    }

    #[test]
    fn the_checkout_badge_sits_at_the_right_and_shortens_the_branch() {
        let mut groups = groups();
        groups[0].status = Some(CheckoutStatus {
            changed: 3,
            ahead: 2,
            behind: 0,
        });
        let (text, buf) = draw(&groups, 28, 2);
        insta::assert_snapshot!(text, @r"
        ▾ monarch  feat/side… ±3 ↑2│
        ▸ $ 1 claude               │
        ");
        assert!(
            !buf[(24, 0)].modifier.contains(Modifier::DIM),
            "badge stands out"
        );
        assert!(
            buf[(11, 0)].modifier.contains(Modifier::DIM),
            "branch is dim"
        );
        let (narrow, _) = draw(&groups, 22, 1);
        assert_eq!(narrow, "▾ monarch  fea… ±3 ↑2│");
        let (tight, _) = draw(&groups, 15, 1);
        assert_eq!(tight, "▾ monarch  fe…│", "no room for the badge");
        let (tiny, _) = draw(&groups, 11, 1);
        assert_eq!(tiny, "▾ monarch │");
    }

    #[test]
    fn agents_needing_attention_stand_out() {
        let mut groups = groups();
        groups[0].rows[1].state = AgentState::Blocked;
        groups[0].rows[1].detail = Some("Allow Bash?".into());
        groups[1].rows[0].state = AgentState::Working;
        let (text, buf) = draw(&groups, 24, 6);
        assert!(text.contains("  ◆ 1 zsh  Allow Bash?"), "{text}");
        assert!(text.contains("  ● 2 htop"), "{text}");
        let blocked = buf[(2, 2)].modifier;
        assert!(blocked.contains(Modifier::REVERSED | Modifier::BOLD));
        assert!(
            !buf[(2, 5)].modifier.contains(Modifier::REVERSED),
            "working"
        );
    }

    #[test]
    fn a_look_sets_the_glyph_colour_and_symbol() {
        let mut groups = groups();
        groups[0].rows[1].state = AgentState::Blocked;
        groups[0].rows[1].look = Some(GlyphLook {
            symbol: '◆',
            color: Some(monarch_core::Rgb::new(200, 160, 0)),
            bold: true,
        });
        groups[1].rows[0].state = AgentState::Working;
        groups[1].rows[0].look = Some(GlyphLook::plain('•'));
        let (text, buf) = draw(&groups, 24, 6);
        assert!(text.contains("  • 2 htop"), "{text}");
        let cell = &buf[(2, 2)];
        assert_eq!(cell.fg, Color::Rgb(200, 160, 0));
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert!(
            !cell.modifier.contains(Modifier::REVERSED),
            "colour replaces reverse video"
        );
    }

    #[test]
    fn clicks_land_on_pane_rows_only_and_what_fits() {
        let groups = groups();
        let sidebar = Sidebar { groups: &groups };
        let targets = sidebar.targets(Rect::new(0, 0, 24, 5));
        assert_eq!(
            targets,
            [
                (PaneId(1), Rect::new(0, 1, 23, 1)),
                (PaneId(2), Rect::new(0, 2, 23, 1))
            ],
            "the scratch group is cut off"
        );
        let (text, _) = draw(&groups, 1, 2);
        assert_eq!(text.trim(), "", "too narrow to draw");
    }
}
