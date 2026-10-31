//! Draws the sidebar: panes grouped under their project and branch, the
//! focused one marked, scrolled to an offset, with a divider on its right
//! edge that carries a scroll thumb when the groups don't all fit.

use monarch_core::PaneId;
use monarch_proto::CheckoutStatus;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use super::{Chrome, SidebarGroup, SidebarRow};

/// Columns pane rows sit in from their heading.
const INDENT: u16 = 2;

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
    /// The colours; headings take the accent.
    pub chrome: Chrome<'a>,
    /// Lines scrolled past at the top.
    pub offset: u16,
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

    /// How many lines the groups take, scrolled or not.
    #[must_use]
    pub fn total_lines(&self) -> u16 {
        u16::try_from(self.all_lines().count()).unwrap_or(u16::MAX)
    }

    /// The line `pane`'s row is on, counted from the top of the groups.
    #[must_use]
    pub fn line_of(&self, pane: PaneId) -> Option<u16> {
        let at = self
            .all_lines()
            .position(|line| matches!(line, Line::Pane(row) if row.pane == pane))?;
        u16::try_from(at).ok()
    }

    /// Every line, from the top of the groups.
    fn all_lines(&self) -> impl Iterator<Item = Line<'a>> + 'a {
        self.groups.iter().enumerate().flat_map(|(i, group)| {
            let gap = (i > 0).then_some(Line::Gap);
            gap.into_iter()
                .chain(std::iter::once(Line::Heading(group)))
                .chain(group.rows.iter().map(Line::Pane))
        })
    }

    /// Every line that fits in `area` past the offset, with its row.
    fn lines(&self, area: Rect) -> impl Iterator<Item = (u16, Line<'a>)> + 'a {
        let lines = self.all_lines().skip(usize::from(self.offset));
        (area.y..area.bottom()).zip(lines)
    }

    /// The divider rows the scroll thumb covers, when the groups overflow
    /// `rows`.
    fn thumb(&self, rows: u16) -> Option<std::ops::Range<u16>> {
        let total = self.total_lines();
        if total <= rows || rows == 0 {
            return None;
        }
        let size = (u32::from(rows) * u32::from(rows) / u32::from(total)).max(1);
        let travel = u32::from(rows) - size;
        let top = u32::from(self.offset.min(total - rows)) * travel / u32::from(total - rows);
        let start = u16::try_from(top).unwrap_or(0);
        Some(start..start + u16::try_from(size).unwrap_or(1))
    }
}

impl Widget for Sidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 {
            return;
        }
        let width = area.width - 1;
        let icons = self
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .any(|r| r.agent.is_some());
        let dim = Style::new().add_modifier(Modifier::DIM);
        let thumb = self.thumb(area.height).unwrap_or_default();
        for y in area.y..area.bottom() {
            let (bar, style) = if thumb.contains(&(y - area.y)) {
                ("┃", Style::new())
            } else {
                ("│", dim)
            };
            buf.set_string(area.x + width, y, bar, style);
        }
        for (y, line) in self.lines(area) {
            let row = Rect::new(area.x, y, width, 1);
            match line {
                Line::Heading(group) => heading(group, self.chrome, row, buf),
                Line::Pane(pane) if width > INDENT => {
                    let indented = Rect::new(row.x + INDENT, y, width - INDENT, 1);
                    pane_line(pane, indented, (icons, self.chrome), buf);
                }
                Line::Pane(_) => {}
                Line::Gap => {}
            }
        }
    }
}

/// `▾ project  branch +3 ↑2`: the branch dim, then the checkout's badge
/// right after it. The branch is shortened so the badge fits; the badge
/// is left out only when even a short branch wouldn't leave room.
fn heading(group: &SidebarGroup, chrome: Chrome<'_>, row: Rect, buf: &mut Buffer) {
    let bold = chrome.highlight();
    let title = format!("▾ {}", group.title);
    let (mut x, _) = buf.set_stringn(row.x, row.y, &title, usize::from(row.width), bold);
    let badge = group.status.map(|s| s.to_string()).unwrap_or_default();
    let badge_width = u16::try_from(Span::raw(badge.as_str()).width()).unwrap_or(u16::MAX);
    let left = row.right().saturating_sub(x);
    let room = left
        .checked_sub(badge_width + 1)
        .filter(|_| !badge.is_empty());
    let detail_room = room.unwrap_or(left);
    let mut gap = 2;
    if let Some(detail) = group.detail.as_deref().filter(|_| detail_room > 2) {
        let dim = Style::new().add_modifier(Modifier::DIM);
        let detail = shorten(detail, usize::from(detail_room - 2));
        (x, _) = buf.set_stringn(x + 2, row.y, &detail, usize::MAX, dim);
        gap = 1;
    }
    if let Some(status) = group.status.filter(|_| room.is_some()) {
        let start = Rect::new(x + gap, row.y, row.right().saturating_sub(x + gap), 1);
        draw_badge(&status, chrome, start, buf);
    }
}

/// `+12 −4 ?2 ↑2 ↓1`, each part in its own style, within `area`.
fn draw_badge(status: &CheckoutStatus, chrome: Chrome<'_>, area: Rect, buf: &mut Buffer) {
    let mut x = area.x;
    for (i, (part, text)) in status.parts().into_iter().enumerate() {
        if i > 0 {
            x += 1;
        }
        let left = usize::from(area.right().saturating_sub(x));
        (x, _) = buf.set_stringn(x, area.y, &text, left, chrome.badge(part));
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

/// `▸ ● ✻ label  detail`: the focus marker, the state's glyph, the
/// agent's icon (a blank keeps rows aligned when some have one and this
/// doesn't) and the pane's activity or directory, dim.
fn pane_line(pane: &SidebarRow, row: Rect, (icons, chrome): (bool, Chrome<'_>), buf: &mut Buffer) {
    let marker = if pane.focused { "▸" } else { " " };
    let glyph = pane.look.map_or(pane.state.glyph(), |l| l.symbol);
    let icon = match pane.agent {
        Some(agent) => format!("{} ", agent.icon()),
        None if icons => "  ".to_owned(),
        None => String::new(),
    };
    let text = format!("{marker} {glyph} {icon}{}", pane.label);
    let width = usize::from(row.width);
    let (x, _) = buf.set_stringn(row.x, row.y, &text, width, row_style(pane));
    if row.width > 2 {
        paint_glyph(pane, &mut buf[(row.x + 2, row.y)]);
    }
    if let Some(color) = pane
        .agent
        .and_then(|a| chrome.agent(a))
        .filter(|_| row.width > 4)
    {
        buf[(row.x + 4, row.y)].set_fg(color);
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
            cell.set_fg(Chrome::color(c));
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
            agent: None,
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
        draw_from(groups, width, height, 0)
    }

    fn draw_from(
        groups: &[SidebarGroup],
        width: u16,
        height: u16,
        offset: u16,
    ) -> (String, Buffer) {
        let mut term = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        term.draw(|f| {
            f.render_widget(
                Sidebar {
                    groups,
                    chrome: Chrome::default(),
                    offset,
                },
                f.area(),
            );
        })
        .expect("draw");
        let buf = term.backend().buffer().clone();
        let text = (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        (text, buf)
    }

    #[test]
    fn agents_get_their_icon_and_other_rows_stay_aligned() {
        let mut groups = groups();
        groups[0].rows[0].agent = Some(monarch_core::AgentKind::Claude);
        groups[1].rows[0].agent = Some(monarch_core::AgentKind::Codex);
        let (text, _) = draw(&groups, 24, 7);
        insta::assert_snapshot!(text, @r"
        ▾ monarch  feat/sidebar│
          ▸ $ ✻ 1 claude       │
            $   1 zsh  src     │
                               │
        ▾ Scratch              │
            $ ⌬ 2 htop  tmp    │
                               │
        ");
        let theme = monarch_core::Theme::dark();
        let mut themed = Buffer::empty(Rect::new(0, 0, 24, 7));
        Sidebar {
            groups: &groups,
            chrome: Chrome::new(Some(&theme)),
            offset: 0,
        }
        .render(themed.area, &mut themed);
        assert_eq!(
            themed[(6, 1)].fg,
            Chrome::color(theme.claude),
            "claude's icon"
        );
        assert_eq!(
            themed[(6, 5)].fg,
            Chrome::color(theme.codex),
            "codex's icon"
        );
        assert_ne!(
            themed[(8, 1)].fg,
            Chrome::color(theme.claude),
            "only the icon"
        );
    }

    #[test]
    fn groups_panes_under_project_and_branch() {
        let (text, buf) = draw(&groups(), 24, 7);
        insta::assert_snapshot!(text, @r"
        ▾ monarch  feat/sidebar│
          ▸ $ 1 claude         │
            $ 1 zsh  src       │
                               │
        ▾ Scratch              │
            $ 2 htop  tmp      │
                               │
        ");
        assert!(buf[(4, 1)].modifier.contains(Modifier::BOLD), "focused");
        assert!(buf[(6, 5)].modifier.contains(Modifier::DIM), "hidden tab");
        assert!(!buf[(6, 2)].modifier.contains(Modifier::DIM), "shown tab");
        let theme = monarch_core::Theme::dark();
        let mut themed = Buffer::empty(Rect::new(0, 0, 24, 7));
        Sidebar {
            groups: &groups(),
            chrome: Chrome::new(Some(&theme)),
            offset: 0,
        }
        .render(themed.area, &mut themed);
        assert_eq!(themed[(2, 0)].fg, Chrome::color(theme.accent), "heading");
    }

    #[test]
    fn the_checkout_badge_follows_the_branch_and_shortens_it() {
        let mut groups = groups();
        groups[0].status = Some(CheckoutStatus {
            added: 3,
            removed: 0,
            untracked: 0,
            ahead: 2,
            behind: 0,
        });
        let (text, buf) = draw(&groups, 28, 2);
        insta::assert_snapshot!(text, @r"
        ▾ monarch  feat/side… +3 ↑2┃
          ▸ $ 1 claude             │
        ");
        let (wide, _) = draw(&groups, 36, 1);
        assert_eq!(
            wide, "▾ monarch  feat/sidebar +3 ↑2      ┃",
            "right after it"
        );
        groups[0].detail = None;
        let (no_branch, _) = draw(&groups, 20, 1);
        assert_eq!(no_branch, "▾ monarch  +3 ↑2   ┃");
        groups[0].detail = Some("feat/sidebar".into());
        assert!(
            !buf[(24, 0)].modifier.contains(Modifier::DIM),
            "badge stands out"
        );
        assert!(
            buf[(11, 0)].modifier.contains(Modifier::DIM),
            "branch is dim"
        );
        let (narrow, _) = draw(&groups, 22, 1);
        assert_eq!(narrow, "▾ monarch  fea… +3 ↑2┃");
        let (tight, _) = draw(&groups, 15, 1);
        assert_eq!(tight, "▾ monarch  fe…┃", "no room for the badge");
        let (tiny, _) = draw(&groups, 11, 1);
        assert_eq!(tiny, "▾ monarch ┃");
    }

    #[test]
    fn badge_parts_take_their_colours() {
        let mut groups = groups();
        groups[0].status = Some(CheckoutStatus {
            added: 3,
            removed: 0,
            untracked: 0,
            ahead: 2,
            behind: 0,
        });
        let theme = monarch_core::Theme::dark();
        let mut themed = Buffer::empty(Rect::new(0, 0, 36, 1));
        Sidebar {
            groups: &groups,
            chrome: Chrome::new(Some(&theme)),
            offset: 0,
        }
        .render(themed.area, &mut themed);
        assert_eq!(themed[(24, 0)].symbol(), "+");
        assert_eq!(
            themed[(24, 0)].fg,
            Chrome::color(theme.added),
            "added is green"
        );
        assert_eq!(themed[(27, 0)].fg, Chrome::color(theme.accent), "ahead");
    }

    #[test]
    fn agents_needing_attention_stand_out() {
        let mut groups = groups();
        groups[0].rows[1].state = AgentState::Blocked;
        groups[0].rows[1].detail = Some("Allow Bash?".into());
        groups[1].rows[0].state = AgentState::Working;
        let (text, buf) = draw(&groups, 26, 6);
        assert!(text.contains("    ◆ 1 zsh  Allow Bash?"), "{text}");
        assert!(text.contains("    ● 2 htop"), "{text}");
        let blocked = buf[(4, 2)].modifier;
        assert!(blocked.contains(Modifier::REVERSED | Modifier::BOLD));
        assert!(
            !buf[(4, 5)].modifier.contains(Modifier::REVERSED),
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
        assert!(text.contains("    • 2 htop"), "{text}");
        let cell = &buf[(4, 2)];
        assert_eq!(cell.fg, ratatui::style::Color::Rgb(200, 160, 0));
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert!(
            !cell.modifier.contains(Modifier::REVERSED),
            "colour replaces reverse video"
        );
    }

    #[test]
    fn clicks_land_on_pane_rows_only_and_what_fits() {
        let groups = groups();
        let sidebar = Sidebar {
            groups: &groups,
            chrome: Chrome::default(),
            offset: 0,
        };
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

    #[test]
    fn scrolled_groups_start_at_the_offset_with_a_thumb() {
        let groups = groups();
        let sidebar = Sidebar {
            groups: &groups,
            chrome: Chrome::default(),
            offset: 3,
        };
        assert_eq!(sidebar.total_lines(), 6);
        assert_eq!(sidebar.line_of(PaneId(3)), Some(5));
        assert_eq!(sidebar.line_of(PaneId(9)), None);
        let (text, buf) = draw_from(&groups, 20, 3, 3);
        insta::assert_snapshot!(text, @r"
                           │
        ▾ Scratch          │
            $ 2 htop  tmp  ┃
        ");
        assert!(
            !buf[(19, 2)].modifier.contains(Modifier::DIM),
            "thumb at the end"
        );
        assert_eq!(
            sidebar.targets(Rect::new(0, 0, 20, 3)),
            [(PaneId(3), Rect::new(0, 2, 19, 1))],
            "clicks follow the scroll"
        );
        let (all, _) = draw(&groups, 20, 6);
        assert!(!all.contains('┃'), "no thumb when everything fits");
    }
}
