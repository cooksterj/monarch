//! Converts a row of cells to style runs and back.

use super::{Run, StyleCache, StyleDef, StyleTable};
use crate::{Cell, CellStyle};

/// Encodes rows as [`Run`]s for the wire and decodes them on the client.
///
/// Trailing blank cells are not sent; the decoder pads the row back to its
/// width. The spacer after a wide cell is implied, and takes the wide cell's
/// style when decoded.
#[derive(Debug, Clone, Copy, Default)]
pub struct RowCodec;

impl RowCodec {
    /// Encodes `cells`, interning styles in `styles` and appending any new
    /// definitions to `defs`.
    #[must_use]
    pub fn encode(cells: &[Cell], styles: &mut StyleTable, defs: &mut Vec<StyleDef>) -> Vec<Run> {
        let blank = Cell::default();
        let end = cells.iter().rposition(|c| *c != blank).map_or(0, |i| i + 1);
        let cells = &cells[..end];
        let mut runs: Vec<Run> = Vec::new();
        let mut i = 0;
        while i < cells.len() {
            let cell = &cells[i];
            let width = cell.width.min(2);
            let style = styles.intern(cell.style, defs);
            let implied_spacer = width == 2 && cells.get(i + 1).is_some_and(|c| c.width == 0);
            match runs.last_mut() {
                Some(run) if cell.combining.is_empty() && extends(run, style, width) => {
                    run.text.push(cell.ch);
                    run.cells += 1;
                }
                _ => runs.push(Run {
                    style,
                    width,
                    cells: 1,
                    text: cell.text(),
                }),
            }
            i += if implied_spacer { 2 } else { 1 };
        }
        runs
    }

    /// Decodes `runs` into exactly `cols` cells.
    #[must_use]
    pub fn decode(runs: &[Run], styles: &StyleCache, cols: u16) -> Vec<Cell> {
        let cols = usize::from(cols);
        let mut row = Vec::with_capacity(cols + 1);
        for run in runs {
            let style = styles.get(run.style);
            if run.is_cluster() {
                let mut chars = run.text.chars();
                let ch = chars.next().unwrap_or(' ');
                push(&mut row, cell(ch, chars.collect(), run.width, style));
            } else {
                for ch in run.text.chars() {
                    push(&mut row, cell(ch, Vec::new(), run.width, style));
                }
            }
            if row.len() >= cols {
                break;
            }
        }
        row.resize(cols, Cell::default());
        row
    }
}

/// Whether a plain cell can join `run`.
fn extends(run: &Run, style: super::StyleId, width: u8) -> bool {
    run.style == style && run.width == width && !run.is_cluster()
}

fn cell(ch: char, combining: Vec<char>, width: u8, style: CellStyle) -> Cell {
    Cell {
        ch,
        combining,
        width,
        style,
    }
}

fn push(row: &mut Vec<Cell>, cell: Cell) {
    let spacer = (cell.width == 2).then(|| Cell {
        style: cell.style,
        ..Cell::spacer()
    });
    row.push(cell);
    row.extend(spacer);
}

#[cfg(test)]
mod tests;
