//! Draws the sidebar: panes grouped under their project and branch, the
//! focused one marked, with a divider on its right edge.

use monarch_core::PaneId;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
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
            let (text, style, detail) = match line {
                Line::Heading(g) => (
                    format!("▾ {}", g.title),
                    Style::new().add_modifier(Modifier::BOLD),
                    g.detail.as_deref(),
                ),
                Line::Pane(row) => {
                    let marker = if row.focused { "▸" } else { " " };
                    let glyph = row.look.map_or(row.state.glyph(), |l| l.symbol);
                    (
                        format!("{marker} {glyph} {}", row.label),
                        row_style(row),
                        row.detail.as_deref(),
                    )
                }
                Line::Gap => continue,
            };
            let (x, _) = buf.set_stringn(area.x, y, &text, usize::from(width), style);
            if let Line::Pane(row) = line
                && width > 2
            {
                paint_glyph(row, &mut buf[(area.x + 2, y)]);
            }
            let left = usize::from(area.x + width - x);
            if let Some(detail) = detail.filter(|_| left > 2) {
                buf.set_stringn(x + 2, y, detail, left - 2, dim);
            }
        }
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
