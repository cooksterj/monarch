//! One sub-agent as the sidebar lists it, under its parent's row.

use monarch_core::AgentState;
use monarch_proto::SubagentStatus;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::GlyphLook;
use super::sidebar::paint_glyph;

/// A sub-agent inside a pane's agent: `├ ● Explore  Grep TODO`, its
/// glyph animated like its parent's. A click on it focuses the parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentRow {
    /// The agent's id for it, which keys its animation.
    pub id: String,
    /// What kind it is (`Explore`).
    pub label: String,
    /// Its state, drawn as a glyph before the label.
    pub state: AgentState,
    /// How its glyph looks this frame; `None` draws the plain glyph.
    pub look: Option<GlyphLook>,
    /// What it is doing or last said, drawn dim after the label.
    pub detail: Option<String>,
}

impl From<&SubagentStatus> for SubagentRow {
    fn from(status: &SubagentStatus) -> Self {
        Self {
            id: status.id.clone(),
            label: status.label().to_owned(),
            state: status.state,
            look: None,
            detail: status.activity.clone(),
        }
    }
}

impl SubagentRow {
    /// Draws it in `row` (where its parent's marker column starts): the
    /// branch under the parent's glyph (`└` for the last child), then its
    /// own glyph under the parent's icon. Dim when its pane's tab isn't
    /// shown.
    pub(crate) fn draw(&self, last: bool, shown: bool, row: Rect, buf: &mut Buffer) {
        let branch = if last { '└' } else { '├' };
        let glyph = self.look.map_or(self.state.glyph(), |l| l.symbol);
        let text = format!("  {branch} {glyph} {}", self.label);
        let base = if shown {
            Style::new()
        } else {
            Style::new().add_modifier(Modifier::DIM)
        };
        let width = usize::from(row.width);
        let (x, _) = buf.set_stringn(row.x, row.y, &text, width, base);
        let dim = Style::new().add_modifier(Modifier::DIM);
        buf.set_stringn(row.x + 2, row.y, branch.to_string(), 1, dim);
        if row.width > 4 {
            paint_glyph(self.look, self.state, &mut buf[(row.x + 4, row.y)]);
        }
        let left = usize::from(row.right().saturating_sub(x));
        if let Some(detail) = self.detail.as_deref().filter(|_| left > 2) {
            buf.set_stringn(x + 2, row.y, detail, left - 2, dim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child(state: AgentState, detail: Option<&str>) -> SubagentRow {
        SubagentRow {
            id: "a1".into(),
            label: "Explore".into(),
            state,
            look: None,
            detail: detail.map(Into::into),
        }
    }

    fn drawn(row: &SubagentRow, last: bool, width: u16) -> (String, Buffer) {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        row.draw(last, true, area, &mut buf);
        let text = (0..width).map(|x| buf[(x, 0)].symbol()).collect();
        (text, buf)
    }

    #[test]
    fn it_hangs_off_its_parent_with_its_state_and_activity() {
        let (text, buf) = drawn(&child(AgentState::Working, Some("Grep TODO")), false, 26);
        assert_eq!(text, "  ├ ● Explore  Grep TODO  ");
        assert!(buf[(2, 0)].modifier.contains(Modifier::DIM), "dim branch");
        assert!(buf[(15, 0)].modifier.contains(Modifier::DIM), "dim detail");
        let (last, buf) = drawn(&child(AgentState::Blocked, None), true, 14);
        assert_eq!(last, "  └ ◆ Explore ");
        assert!(
            buf[(4, 0)].modifier.contains(Modifier::REVERSED),
            "a question stands out without colour"
        );
    }

    #[test]
    fn it_comes_from_the_status_its_agent_reported() {
        let status = SubagentStatus {
            id: "a9".into(),
            kind: String::new(),
            state: AgentState::Working,
            activity: Some("Read a.rs".into()),
        };
        let row = SubagentRow::from(&status);
        assert_eq!(
            (row.id.as_str(), row.label.as_str(), row.detail.as_deref()),
            ("a9", "subagent", Some("Read a.rs"))
        );
        assert_eq!(row.look, None, "dressed per frame");
    }

    #[test]
    fn a_narrow_sidebar_cuts_it_short() {
        let (text, _) = drawn(&child(AgentState::Done, Some("Found 3")), true, 9);
        assert_eq!(text, "  └ ✓ Exp");
        let (tiny, _) = drawn(&child(AgentState::Done, None), true, 3);
        assert_eq!(tiny, "  └", "the glyph needs more room");
    }
}
