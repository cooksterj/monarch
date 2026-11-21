//! Draws the file viewer over the panes: line numbers in a gutter, the
//! code coloured by the theme's syntax tokens, the cursor's line and the
//! selection marked in the gutter so the code keeps its colours.

use monarch_core::Theme;
use monarch_proto::CodeLine;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthChar;

use super::{Chrome, PopupFrame};
use crate::FileViewer;

const KEYS: &str = " j/k move · V select · a ask · y copy · e edit · space/b page · esc close ";
/// Columns a tab takes, as the review draws them.
const TAB: usize = 4;

/// The file viewer as drawn.
#[derive(Debug, Clone, Copy)]
pub struct FileViewerView<'a> {
    /// What to draw.
    pub viewer: &'a FileViewer,
    /// Colours for code and chrome; `None` draws without (`NO_COLOR`).
    pub theme: Option<&'a Theme>,
}

impl FileViewerView<'_> {
    /// How many lines fit when drawn `height` rows tall (for paging).
    #[must_use]
    pub fn code_rows(height: u16) -> usize {
        usize::from(height.saturating_sub(2))
    }

    fn title(&self) -> String {
        let path = self.viewer.path();
        match self.viewer.file() {
            Some(Ok(file)) => {
                let lang = file.language.as_deref().unwrap_or("text");
                let cut = if file.truncated { " · cut short" } else { "" };
                format!(" {path} · {lang} · {} lines{cut} ", file.lines.len())
            }
            _ => format!(" {path} "),
        }
    }

    /// The gutter for line `i` (from 0): a marker for the cursor or the
    /// selection, then its number right-aligned in `digits` columns.
    fn gutter(&self, i: usize, digits: usize) -> (String, Style) {
        let chrome = Chrome::new(self.theme);
        let selected = self
            .viewer
            .selection()
            .is_some_and(|(a, b)| (a..=b).contains(&i));
        let (mark, style) = if selected {
            ('▌', chrome.highlight().add_modifier(Modifier::REVERSED))
        } else if i == self.viewer.cursor() {
            ('›', chrome.highlight())
        } else {
            (' ', Style::new().add_modifier(Modifier::DIM))
        };
        (format!("{mark}{:>digits$} ", i + 1), style)
    }

    /// Draws one line of code into `row`, cut at its right edge.
    fn draw_code(&self, line: &CodeLine, row: Rect, buf: &mut Buffer) {
        let (x, y, width) = (row.x, row.y, usize::from(row.width));
        let mut used = 0;
        for span in &line.spans {
            let style = self
                .theme
                .and_then(|t| t.syntax(span.token))
                .map_or(Style::new(), |rgb| Style::new().fg(Chrome::color(rgb)));
            for c in span.text.chars() {
                let (text, w) = match c {
                    '\t' => (" ".repeat(TAB - used % TAB), TAB - used % TAB),
                    c => (c.to_string(), c.width().unwrap_or(0)),
                };
                if used + w > width {
                    return;
                }
                let col = x + u16::try_from(used).unwrap_or(u16::MAX);
                buf.set_stringn(col, y, &text, w, style);
                used += w;
            }
        }
    }
}

impl Widget for FileViewerView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 4 {
            return;
        }
        let inner = PopupFrame::new(Chrome::new(self.theme))
            .title(self.title())
            .footer(KEYS)
            .draw(area, buf);
        let dim = Style::new().add_modifier(Modifier::DIM);
        let file = match self.viewer.file() {
            None | Some(Err(_)) => {
                let why = match self.viewer.file() {
                    Some(Err(why)) => why.as_str(),
                    _ => "reading…",
                };
                let room = usize::from(inner.width.saturating_sub(1));
                buf.set_stringn(inner.x + 1, inner.y, why, room, dim);
                return;
            }
            Some(Ok(file)) => file,
        };
        let digits = file.lines.len().max(1).to_string().len();
        let code_x = inner.x + u16::try_from(digits + 2).unwrap_or(u16::MAX);
        let room = inner.right().saturating_sub(code_x);
        let rows = (inner.y..inner.bottom()).zip(self.viewer.top()..);
        for (y, i) in rows {
            let Some(line) = file.lines.get(i) else {
                break;
            };
            let (gutter, style) = self.gutter(i, digits);
            buf.set_stringn(inner.x, y, gutter, digits + 2, style);
            self.draw_code(line, Rect::new(code_x, y, room, 1), buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use monarch_core::{PaneId, SyntaxToken};
    use monarch_proto::{CodeFile, CodeSpan};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::OverlayKey;

    fn viewer(lines: &[&[(SyntaxToken, &str)]]) -> FileViewer {
        let mut v = FileViewer::new(PaneId(1), "src/a.rs");
        let line = |spans: &&[(SyntaxToken, &str)]| CodeLine {
            spans: spans.iter().map(|(k, t)| CodeSpan::new(*k, *t)).collect(),
        };
        v.load(Ok(CodeFile {
            path: "src/a.rs".into(),
            language: Some("rust".into()),
            lines: lines.iter().map(line).collect(),
            truncated: false,
        }));
        v
    }

    fn draw(v: &FileViewer, theme: Option<&Theme>, w: u16, h: u16) -> (String, Buffer) {
        let mut term = Terminal::new(TestBackend::new(w, h)).expect("terminal");
        term.draw(|f| f.render_widget(FileViewerView { viewer: v, theme }, f.area()))
            .expect("draw");
        let buf = term.backend().buffer().clone();
        let text = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, buf)
    }

    #[test]
    fn numbers_the_lines_marks_the_selection_and_expands_tabs() {
        let mut v = viewer(&[
            &[(SyntaxToken::Keyword, "fn"), (SyntaxToken::Plain, " a() {")],
            &[(SyntaxToken::Plain, "\tlet x = 1;")],
            &[(SyntaxToken::Plain, "}")],
        ]);
        v.key(OverlayKey::Char('V'));
        v.key(OverlayKey::Char('j'));
        insta::assert_snapshot!(draw(&v, None, 48, 7).0, @r"
        ╭ src/a.rs · rust · 3 lines ───────────────────╮
        │▌1 fn a() {                                   │
        │▌2     let x = 1;                             │
        │ 3 }                                          │
        │                                              │
        │                                              │
        ╰ j/k move · V select · a ask · y copy · e edit╯
        ");
    }

    #[test]
    fn code_takes_the_themes_syntax_colours_and_long_lines_are_cut() {
        let v = viewer(&[&[
            (SyntaxToken::Keyword, "fn"),
            (SyntaxToken::Plain, " main_function_name"),
        ]]);
        let theme = Theme::dark();
        let (text, buf) = draw(&v, Some(&theme), 22, 4);
        assert!(text.contains("›1 fn main_function"), "{text}");
        assert!(!text.contains("name"), "cut at the border: {text}");
        let keyword = Chrome::color(theme.syntax_keyword);
        assert_eq!(buf[(4, 1)].fg, keyword, "`fn` in the keyword colour");
        assert_ne!(buf[(7, 1)].fg, keyword, "plain text is not");
    }

    #[test]
    fn loading_and_errors_say_so() {
        let mut v = FileViewer::new(PaneId(1), "bin");
        assert!(draw(&v, None, 30, 4).0.contains("reading…"));
        v.load(Err("bin is a binary file".into()));
        assert!(draw(&v, None, 30, 4).0.contains("bin is a binary file"));
        assert_eq!(draw(&v, None, 10, 3).0.trim(), "", "too small to draw");
        assert_eq!(FileViewerView::code_rows(10), 8);
    }
}
