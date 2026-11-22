//! The right side of the review pane: a file's name, then its diff from
//! the scroll position. Highlighted lines take the theme's syntax colours
//! on a tint of the added or removed colour; others are coloured by kind.

use monarch_core::{Rgb, Theme};

use crate::Chrome;
use monarch_proto::{DiffLine, DiffLineKind, ReviewFile};
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
        let y = area.y + row;
        match theme {
            Some(theme) if !line.spans.is_empty() => {
                draw_code(line, theme, Rect::new(area.x, y, area.width, 1), buf);
            }
            _ => {
                let text = format!("{}{}", line.kind.marker(), printable(&line.text));
                buf.set_stringn(area.x, y, text, width, style(line.kind, theme));
            }
        }
    }
}

/// How much of the added or removed colour tints its lines' background.
const TINT: f32 = 0.18;

/// A highlighted line: its marker in the kind's colour, then its code in
/// syntax colours, the whole row on the kind's tint.
fn draw_code(line: &DiffLine, theme: &Theme, row: Rect, buf: &mut Buffer) {
    let tint = match line.kind {
        DiffLineKind::Added => Some(theme.added),
        DiffLineKind::Removed => Some(theme.removed),
        _ => None,
    };
    let base = tint.map_or(Style::new(), |rgb| {
        Style::new().bg(Chrome::color(theme.popup.lerp(rgb, TINT)))
    });
    let right = row.right();
    buf.set_style(row, base);
    let marker = style(line.kind, Some(theme)).patch(base);
    let (mut x, _) = buf.set_stringn(row.x, row.y, line.kind.marker(), 1, marker);
    for span in &line.spans {
        let room = usize::from(right.saturating_sub(x));
        if room == 0 {
            break;
        }
        let fg = theme.syntax(span.token).map(Chrome::color);
        let style = fg.map_or(base, |fg| base.fg(fg));
        (x, _) = buf.set_stringn(x, row.y, printable(&span.text), room, style);
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
    fn highlighted_lines_take_syntax_colours_on_their_tint() {
        use monarch_core::SyntaxToken;
        use monarch_proto::{CodeSpan, FileChange};

        let theme = Theme::dark();
        let mut file = ReviewFile::new("a.rs", FileChange::Modified);
        let mut added = DiffLine::new(DiffLineKind::Added, "let x");
        added.spans = vec![
            CodeSpan::new(SyntaxToken::Keyword, "let"),
            CodeSpan::new(SyntaxToken::Plain, " x"),
        ];
        file.lines = vec![added, DiffLine::new(DiffLineKind::Context, "plain")];
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 3));
        draw(&file, 0, Some(&theme), buf.area, &mut buf);
        let row: String = (0..12).map(|x| buf[(x, 1)].symbol()).collect();
        assert_eq!(row.trim_end(), "+let x");
        assert_eq!(buf[(0, 1)].fg, Chrome::color(theme.added), "the marker");
        assert_eq!(buf[(1, 1)].fg, Chrome::color(theme.syntax_keyword));
        let tint = Chrome::color(theme.popup.lerp(theme.added, TINT));
        assert!((0..12).all(|x| buf[(x, 1)].bg == tint), "the whole row");
        assert_ne!(buf[(0, 2)].bg, tint, "context has no tint");
        let mut plain = Buffer::empty(Rect::new(0, 0, 12, 3));
        draw(&file, 0, None, plain.area, &mut plain);
        assert_eq!(plain[(1, 1)].fg, ratatui::style::Color::Reset, "NO_COLOR");
    }

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
