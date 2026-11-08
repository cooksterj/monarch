//! A ratatui backend for the host terminal that re-homes the cursor after
//! every glyph whose width the host might measure differently.
//!
//! Crossterm's backend prints consecutive changed cells without moving the
//! cursor between them, trusting the host to advance exactly one column
//! per single-width symbol. Hosts disagree with the emulator about some
//! glyphs: iTerm2 draws every `U+FE0F` emoji two cells wide where
//! alacritty advanced one, so from there on the run lands one cell to the
//! right, and the misplaced characters survive every later frame because
//! the diff believes those cells are already right. This backend moves the
//! cursor explicitly after each non-ASCII symbol, so a disagreement can
//! touch only the glyph's own cells (Terminal Fidelity Contract G1).

use std::io::{self, Stdout, Write};

use ratatui::Terminal;
use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::{Cell, CellWidth};
use ratatui::crossterm::cursor::MoveTo;
use ratatui::crossterm::style::Print;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::crossterm::{execute, queue};
use ratatui::layout::{Position, Size};

mod pen;
use pen::Pen;

/// Emoji presentation selector. Emulators that follow `unicode-width` per
/// character advance one cell for `X U+FE0F`; most hosts draw two.
const VS16: char = '\u{FE0F}';

/// How far a symbol may reach on the host beyond what the buffer believes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// One cell everywhere; still followed by an explicit move if non-ASCII.
    Narrow,
    /// Two cells in the buffer (and the diff skipped the spacer after it).
    Wide,
    /// One cell in the pane's emulator, likely two on the host.
    Emoji,
}

impl Reach {
    fn of(symbol: &str) -> Self {
        if symbol.contains(VS16) {
            Self::Emoji
        } else if symbol.cell_width() > 1 {
            Self::Wide
        } else {
            Self::Narrow
        }
    }
}

/// Crossterm's backend with a cursor that is re-homed after every glyph
/// the host could size differently.
#[derive(Debug)]
pub struct HostBackend<W: Write = Stdout> {
    inner: CrosstermBackend<W>,
}

impl HostBackend<Stdout> {
    /// Raw mode, the alternate screen and a terminal drawing through this
    /// backend. A panic restores the host first, as `ratatui::init` does;
    /// `ratatui::restore` undoes the rest.
    ///
    /// # Errors
    /// The terminal refuses raw mode or cannot be written.
    pub fn init() -> io::Result<Terminal<Self>> {
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            ratatui::restore();
            hook(info);
        }));
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Terminal::new(Self::new(io::stdout()))
    }
}

impl<W: Write> HostBackend<W> {
    /// A backend writing to `writer`.
    pub fn new(writer: W) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
        }
    }

    /// Prints one cell, moving the cursor first unless it is known to sit
    /// there. Afterwards the cursor is trusted only after ASCII.
    fn put(
        &mut self,
        at: Position,
        cell: &Cell,
        pen: &mut Pen,
        last: &mut Option<Position>,
    ) -> io::Result<()> {
        if !matches!(*last, Some(p) if p.y == at.y && p.x + 1 == at.x) {
            queue!(self.inner, MoveTo(at.x, at.y))?;
        }
        pen.apply(cell, &mut self.inner)?;
        queue!(self.inner, Print(cell.symbol()))?;
        *last = cell.symbol().is_ascii().then_some(at);
        Ok(())
    }
}

impl<W: Write> Write for HostBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Write::flush(&mut self.inner)
    }
}

impl<W: Write> Backend for HostBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut pen = Pen::default();
        let mut last = None;
        let mut content = content.peekable();
        while let Some((x, y, cell)) = content.next() {
            let right = Position::new(x.saturating_add(1), y);
            match Reach::of(cell.symbol()) {
                Reach::Narrow => {}
                // The spacer the diff skipped: blank it before the glyph, so
                // a host drawing the glyph narrow shows nothing stale there
                // and one drawing it wide covers the blank.
                Reach::Wide if right.x > x => {
                    let mut spacer = Cell::new(" ");
                    spacer.set_style(cell.style());
                    self.put(right, &spacer, &mut pen, &mut last)?;
                }
                // The neighbour changed too: draw it first for the same
                // reason, leaving its content where the emulator put it.
                Reach::Emoji
                    if content
                        .peek()
                        .is_some_and(|&(nx, ny, _)| (nx, ny) == (right.x, right.y)) =>
                {
                    let (_, _, neighbour) = content.next().expect("invariant: peeked");
                    self.put(right, neighbour, &mut pen, &mut last)?;
                }
                Reach::Wide | Reach::Emoji => {}
            }
            self.put(Position::new(x, y), cell, &mut pen, &mut last)?;
        }
        pen.reset(&mut self.inner)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn size(&self) -> io::Result<Size> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use ratatui::style::{Modifier, Style};

    use super::*;

    /// A writer the test can read back after the backend has it.
    #[derive(Clone, Default)]
    struct Shared(Rc<RefCell<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Draws `cells` on row 0 and returns what was written.
    fn draw_cells(cells: &[(u16, Cell)]) -> String {
        let shared = Shared::default();
        let mut backend = HostBackend::new(shared.clone());
        backend
            .draw(cells.iter().map(|(x, c)| (*x, 0, c)))
            .expect("draw");
        Write::flush(&mut backend).expect("flush");
        let bytes = shared.0.borrow();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Draws plain `(symbol, x)` cells on row 0.
    fn draw(cells: &[(&'static str, u16)]) -> String {
        let owned: Vec<(u16, Cell)> = cells.iter().map(|&(s, x)| (x, Cell::new(s))).collect();
        draw_cells(&owned)
    }

    fn at(text: &str, needle: &str) -> usize {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle:?} not in {text:?}"))
    }

    #[test]
    fn an_ascii_run_moves_the_cursor_once() {
        let out = draw(&[("a", 0), ("b", 1), ("c", 2)]);
        assert_eq!(out.matches("\x1b[1;1H").count(), 1, "{out:?}");
        assert!(
            !out.contains("\x1b[1;2H") && !out.contains("\x1b[1;3H"),
            "{out:?}"
        );
        assert!(at(&out, "a") < at(&out, "b") && at(&out, "b") < at(&out, "c"));
    }

    #[test]
    fn the_cell_after_a_non_ascii_glyph_is_moved_to() {
        let out = draw(&[("✓", 0), ("G", 1), ("e", 2)]);
        assert!(
            at(&out, "✓") < at(&out, "\x1b[1;2H"),
            "move after the glyph: {out:?}"
        );
        assert!(at(&out, "\x1b[1;2H") < at(&out, "G"));
        assert!(!out.contains("\x1b[1;3H"), "ascii stays a run: {out:?}");
    }

    #[test]
    fn an_emoji_with_vs16_has_its_neighbour_drawn_first() {
        let out = draw(&[("⚠\u{FE0F}", 0), ("x", 1), ("y", 2)]);
        assert!(
            at(&out, "x") < at(&out, "⚠"),
            "neighbour before the glyph: {out:?}"
        );
        assert!(
            at(&out, "\x1b[1;1H") > at(&out, "x"),
            "then home to the glyph: {out:?}"
        );
        assert!(at(&out, "⚠") < at(&out, "\x1b[1;3H") && at(&out, "\x1b[1;3H") < at(&out, "y"));
    }

    #[test]
    fn an_emoji_whose_neighbour_did_not_change_is_just_drawn() {
        let out = draw(&[("⚠\u{FE0F}", 0), ("y", 2)]);
        assert!(!out.contains("\x1b[1;2H"), "{out:?}");
        assert!(at(&out, "⚠") < at(&out, "\x1b[1;3H"));
    }

    #[test]
    fn a_wide_glyph_blanks_its_spacer_first() {
        let out = draw(&[("中", 0), ("z", 2)]);
        assert!(
            at(&out, "\x1b[1;2H ") < at(&out, "中"),
            "spacer before the glyph: {out:?}"
        );
        assert!(at(&out, "中") < at(&out, "\x1b[1;3H") && at(&out, "\x1b[1;3H") < at(&out, "z"));
    }

    #[test]
    fn the_spacer_takes_the_glyphs_style() {
        let mut wide = Cell::new("中");
        wide.set_style(Style::default().bg(ratatui::style::Color::Red));
        let out = draw_cells(&[(0, wide)]);
        assert!(
            at(&out, "48;5;1m ") < at(&out, "中"),
            "red, the spacer, then the glyph: {out:?}"
        );
    }

    #[test]
    fn styles_are_sent_once_per_change_and_reset_at_the_end() {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut a = Cell::new("a");
        a.set_style(bold);
        let mut b = Cell::new("b");
        b.set_style(bold);
        let out = draw_cells(&[(0, a), (1, b)]);
        assert_eq!(out.matches("\x1b[1m").count(), 1, "{out:?}");
        assert!(out.ends_with("\x1b[0m"), "{out:?}");
    }
}
