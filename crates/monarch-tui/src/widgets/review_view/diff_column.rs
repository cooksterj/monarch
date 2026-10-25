//! The right side of the review pane: a file's name, then its diff from
//! the scroll position, coloured by line kind.

use monarch_core::{Rgb, Theme};

use crate::Chrome;
use monarch_proto::{DiffLineKind, ReviewFile};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

/// Draws `file` into `area`, its diff starting at line `scroll`.
pub(super) fn draw(
    file: &ReviewFile,
    scroll: usize,
    theme: Option<&Theme>,
    area: Rect,
    buf: &mut Buffer,
) {
    let width = usize::from(area.width);
    let bold = Style::new().add_modifier(Modifier::BOLD);
    buf.set_stringn(area.x, area.y, &file.path, width, bold);
    let shown = usize::from(area.height.saturating_sub(1));
    for (row, line) in (1u16..).zip(file.lines.iter().skip(scroll).take(shown)) {
        let text = format!("{}{}", line.kind.marker(), printable(&line.text));
        buf.set_stringn(area.x, area.y + row, text, width, style(line.kind, theme));
    }
}

fn style(kind: DiffLineKind, theme: Option<&Theme>) -> Style {
    let fg = |pick: fn(&Theme) -> Rgb| {
        theme.map_or(Style::new(), |t| Style::new().fg(Chrome::color(pick(t))))
    };
    match kind {
        DiffLineKind::Added => fg(|t| t.added),
        DiffLineKind::Removed => fg(|t| t.removed),
        DiffLineKind::Hunk => fg(|t| t.hunk).add_modifier(Modifier::DIM),
        DiffLineKind::Note => Style::new().add_modifier(Modifier::DIM | Modifier::ITALIC),
        _ => Style::new(),
    }
}

/// `text` safe to put in cells: tabs as four spaces, other controls dropped.
fn printable(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            '\t' => "    ".chars().collect::<Vec<_>>(),
            c if c.is_control() => Vec::new(),
            c => vec![c],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_expand_and_controls_go() {
        assert_eq!(printable("\tx\u{1b}[31my"), "    x[31my");
    }

    #[test]
    fn colours_come_from_the_theme_or_not_at_all() {
        let theme = Theme::dark();
        let added = style(DiffLineKind::Added, Some(&theme));
        assert_eq!(added.fg, Some(Chrome::color(theme.added)));
        assert_eq!(style(DiffLineKind::Added, None).fg, None, "NO_COLOR");
        assert_eq!(style(DiffLineKind::Context, Some(&theme)), Style::new());
    }
}
