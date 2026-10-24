//! [`Engine`] implemented with `alacritty_terminal`'s `Term` and vte's
//! `Processor`.

use std::sync::mpsc::Receiver;
use std::time::Instant;

use alacritty_terminal::event::Event;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Osc52, Term, TermDamage};
use alacritty_terminal::vte::ansi::{CursorShape as AlacShape, Processor};
use monarch_proto::{Cell, Cursor, CursorShape, PaneState, Screen, ScreenSize, TerminalModes};

use super::cell_convert::CellConverter;
use super::event_translator::{EventTranslator, TranslateContext};
use super::listener::PaneListener;
use super::mode_convert::ModeConverter;
use crate::{Damage, Engine, EngineEvent, HostProfile, ScrollRequest};

/// The default engine.
pub struct AlacrittyEngine {
    term: Term<PaneListener>,
    processor: Processor,
    events: Receiver<Event>,
    host: HostProfile,
}

impl AlacrittyEngine {
    /// An engine with `scrollback` lines of history.
    #[must_use]
    pub fn new(size: ScreenSize, scrollback: usize) -> Self {
        let config = Config {
            scrolling_history: scrollback,
            // Without this alacritty ignores kitty keyboard pushes and
            // Shift+Enter in Claude Code silently stops working (G2).
            kitty_keyboard: true,
            osc52: Osc52::OnlyCopy,
            ..Config::default()
        };
        let (listener, events) = PaneListener::new();
        let dims = TermSize::new(usize::from(size.cols), usize::from(size.rows));
        Self {
            term: Term::new(config, &dims, listener),
            processor: Processor::new(),
            events,
            host: HostProfile::default(),
        }
    }

    fn drain_events(&mut self, out: &mut Vec<EngineEvent>) {
        let ctx = TranslateContext {
            pane_colors: self.term.colors(),
            host: &self.host,
            size: grid_size(&self.term),
        };
        out.extend(
            self.events
                .try_iter()
                .filter_map(|e| EventTranslator::translate(e, &ctx)),
        );
    }

    fn cursor(&self, offset: i32, rows: u16) -> Cursor {
        let content = self.term.renderable_content();
        let row = content.cursor.point.line.0 + offset;
        let in_view = (0..i32::from(rows)).contains(&row);
        let shape = match content.cursor.shape {
            AlacShape::Underline => CursorShape::Underline,
            AlacShape::Beam => CursorShape::Beam,
            _ => CursorShape::Block,
        };
        Cursor {
            row: u16::try_from(row.max(0)).unwrap_or(0),
            col: u16::try_from(content.cursor.point.column.0).unwrap_or(u16::MAX),
            shape,
            blinking: self.term.cursor_style().blinking,
            visible: in_view && content.cursor.shape != AlacShape::Hidden,
        }
    }
}

fn grid_size<T>(term: &Term<T>) -> ScreenSize {
    let clamp = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    ScreenSize::new(clamp(term.columns()), clamp(term.screen_lines()))
}

impl Engine for AlacrittyEngine {
    fn feed(&mut self, bytes: &[u8], events: &mut Vec<EngineEvent>) {
        self.processor.advance(&mut self.term, bytes);
        self.drain_events(events);
    }

    fn resize(&mut self, size: ScreenSize) {
        self.term.resize(TermSize::new(
            usize::from(size.cols),
            usize::from(size.rows),
        ));
    }

    fn size(&self) -> ScreenSize {
        grid_size(&self.term)
    }

    fn screen(&self) -> Screen {
        let size = self.size();
        let mut screen = Screen::blank(size);
        let content = self.term.renderable_content();
        let offset = i32::try_from(content.display_offset).unwrap_or(i32::MAX);
        for indexed in content.display_iter {
            let Ok(row) = usize::try_from(indexed.point.line.0 + offset) else {
                continue;
            };
            if let Some(cell) = screen
                .rows
                .get_mut(row)
                .and_then(|r| r.get_mut(indexed.point.column.0))
            {
                *cell = CellConverter::cell(indexed.cell);
            }
        }
        let state = self.pane_state();
        screen.cursor = state.cursor;
        screen.modes = state.modes;
        screen.display_offset = state.display_offset;
        screen.history = state.history;
        screen
    }

    fn rows(&self, lines: &[u16], out: &mut Vec<Vec<Cell>>) {
        let grid = self.term.grid();
        let offset = i32::try_from(grid.display_offset()).unwrap_or(i32::MAX);
        let (cols, screen_lines) = (grid.columns(), grid.screen_lines());
        for &line in lines {
            if usize::from(line) >= screen_lines {
                out.push(vec![Cell::default(); cols]);
                continue;
            }
            let row = &grid[Line(i32::from(line) - offset)];
            out.push(
                (0..cols)
                    .map(|c| CellConverter::cell(&row[Column(c)]))
                    .collect(),
            );
        }
    }

    fn pane_state(&self) -> PaneState {
        let grid = self.term.grid();
        let offset = i32::try_from(grid.display_offset()).unwrap_or(i32::MAX);
        PaneState {
            cursor: self.cursor(offset, self.size().rows),
            modes: self.modes(),
            display_offset: u32::try_from(grid.display_offset()).unwrap_or(u32::MAX),
            history: u32::try_from(grid.history_size()).unwrap_or(u32::MAX),
        }
    }

    fn take_damage(&mut self) -> Damage {
        let damage = match self.term.damage() {
            TermDamage::Full => Damage::Full,
            TermDamage::Partial(lines) => {
                Damage::Rows(lines.filter_map(|l| u16::try_from(l.line).ok()).collect())
            }
        };
        self.term.reset_damage();
        damage.merge(Damage::none())
    }

    fn modes(&self) -> TerminalModes {
        ModeConverter::modes(*self.term.mode())
    }

    fn scroll(&mut self, request: ScrollRequest) {
        self.term.scroll_display(match request {
            ScrollRequest::Lines(n) => Scroll::Delta(n),
            ScrollRequest::PageUp => Scroll::PageUp,
            ScrollRequest::PageDown => Scroll::PageDown,
            ScrollRequest::Top => Scroll::Top,
            ScrollRequest::Bottom => Scroll::Bottom,
        });
    }

    fn sync_deadline(&self) -> Option<Instant> {
        self.processor.sync_timeout().sync_timeout()
    }

    fn end_sync(&mut self, events: &mut Vec<EngineEvent>) {
        self.processor.stop_sync(&mut self.term);
        self.drain_events(events);
    }

    fn set_host_profile(&mut self, profile: HostProfile) {
        self.host = profile;
    }
}

#[cfg(test)]
mod tests;
