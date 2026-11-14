//! Draws the agent info over the panes: a box near the top, as tall as
//! its page needs, with the position and keys in its bottom border.

use monarch_core::UnixMillis;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use super::{Chrome, PageView};
use crate::AgentInfo;

/// Widest the box gets.
const WIDTH: u16 = 78;

/// The agent info as drawn.
#[derive(Debug, Clone, Copy)]
pub struct AgentInfoView<'a> {
    /// What to draw.
    pub info: &'a AgentInfo,
    /// The time, to age the states against.
    pub now: UnixMillis,
    /// The colours.
    pub chrome: Chrome<'a>,
}

impl AgentInfoView<'_> {
    /// Where a box with `lines` of body goes inside `area`: centred, a
    /// little below the top, no taller than its page.
    #[must_use]
    pub fn placement(area: Rect, lines: usize) -> Rect {
        let wanted = u16::try_from(lines).unwrap_or(u16::MAX).saturating_add(2);
        let height = wanted.clamp(3, area.height.max(3)).min(area.height);
        let width = WIDTH.min(area.width);
        let top = area.height.saturating_sub(height) / 5;
        Rect::new(
            area.x + (area.width - width) / 2,
            area.y + top,
            width,
            height,
        )
    }

    /// Lines a box in `area` shows at once for a page of `lines`.
    #[must_use]
    pub fn body_rows(area: Rect, lines: usize) -> usize {
        PageView::body_rows(Self::placement(area, lines))
    }
}

impl Widget for AgentInfoView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.info.lines(self.now);
        let outer = Self::placement(area, lines.len());
        let mut page = PageView {
            title: self.info.title(),
            footer: String::new(),
            lines: lines.iter().collect(),
            scroll: self.info.clamped_scroll(self.now),
            empty: "",
            chrome: self.chrome,
        };
        let place = page.place(PageView::body_rows(outer));
        page.footer = format!("{place} ↑↓ scroll · r refresh · esc close ");
        page.render(outer, buf);
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{AgentState, PaneId};
    use monarch_proto::{AgentStatus, PaneExplanation, StatusSource};

    use super::*;

    fn draw(info: &AgentInfo, w: u16, h: u16) -> Vec<String> {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        AgentInfoView {
            info,
            now: UnixMillis(100_000),
            chrome: Chrome::default(),
        }
        .render(buf.area, &mut buf);
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .map(|s| s.trim_end().to_owned())
            .collect()
    }

    #[test]
    fn the_box_fits_its_page_and_says_when_it_is_waiting() {
        let mut info = AgentInfo::new(PaneId(2), "codex");
        let drawn = draw(&info, 60, 12);
        assert_eq!(drawn[0], "", "a little below the top");
        assert!(
            drawn[1].starts_with("┌ agent info · pane 2 codex "),
            "{drawn:?}"
        );
        assert!(drawn[2].contains("asking the server…"));
        assert!(drawn[3].starts_with("└ ↑↓ scroll · r refresh · esc close "));
        assert_eq!(drawn[4], "", "three rows: border, line, border");
        info.load(Some(Box::new(PaneExplanation {
            pane: PaneId(2),
            tab: 1,
            program: Some("codex".into()),
            status: AgentStatus {
                state: AgentState::Working,
                activity: Some("Edit a.rs".into()),
                agent: None,
            },
            source: StatusSource::Notification,
            manifest: None,
            screen_state: None,
            winner: None,
            rules: Vec::new(),
            transitions: Vec::new(),
            subagents: Vec::new(),
        })));
        insta::assert_snapshot!(draw(&info, 60, 8).join("\n"));
        assert_eq!(AgentInfoView::body_rows(Rect::new(0, 0, 60, 8), 20), 6);
        assert_eq!(AgentInfoView::body_rows(Rect::new(0, 0, 60, 30), 4), 4);
    }
}
