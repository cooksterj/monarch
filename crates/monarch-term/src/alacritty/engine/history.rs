//! Reading and searching the whole grid, scrollback included, by absolute
//! line (line 0 is the oldest scrollback line).

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Boundary, Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::Term;
use alacritty_terminal::term::search::RegexSearch;
use monarch_proto::{GridPoint, SelectShape};

/// Characters with a meaning in a regex, escaped so a query is plain text.
const REGEX_META: &str = "\\.+*?()|[]{}^$#&-~";

/// The grid point `p` as an alacritty point, clamped onto the grid.
fn point<T>(term: &Term<T>, p: GridPoint) -> Point {
    let history = i64::try_from(term.history_size()).unwrap_or(i64::MAX);
    let last_line = i64::try_from(term.screen_lines()).unwrap_or(i64::MAX) - 1;
    let line = (p.line - history).clamp(-history, last_line);
    let col = usize::from(p.col).min(term.columns().saturating_sub(1));
    Point::new(Line(i32::try_from(line).unwrap_or(0)), Column(col))
}

fn grid_point<T>(term: &Term<T>, p: Point) -> GridPoint {
    let history = i64::try_from(term.history_size()).unwrap_or(i64::MAX);
    GridPoint {
        line: i64::from(p.line.0) + history,
        col: u16::try_from(p.column.0).unwrap_or(u16::MAX),
    }
}

/// The text between `start` and `end` in `shape`, using alacritty's own
/// selection rules (wrapped lines joined, wide characters whole). The
/// terminal's selection is put back afterwards.
pub(super) fn text_range<T>(
    term: &mut Term<T>,
    start: GridPoint,
    end: GridPoint,
    shape: SelectShape,
) -> String {
    let ty = match shape {
        SelectShape::Chars => SelectionType::Simple,
        SelectShape::Lines => SelectionType::Lines,
        SelectShape::Block => SelectionType::Block,
    };
    let (a, b) = (point(term, start.min(end)), point(term, start.max(end)));
    let mut selection = Selection::new(ty, a, Side::Left);
    selection.update(b, Side::Right);
    let saved = term.selection.replace(selection);
    let text = term.selection_to_string().unwrap_or_default();
    term.selection = saved;
    text
}

/// The next place `query` appears after `from` (before it, if
/// `backward`), wrapping around the grid.
pub(super) fn search<T>(
    term: &Term<T>,
    query: &str,
    from: GridPoint,
    backward: bool,
) -> Option<(GridPoint, GridPoint)> {
    if query.is_empty() {
        return None;
    }
    let mut regex = RegexSearch::new(&escape(query)).ok()?;
    let origin = point(term, from);
    let (direction, origin) = if backward {
        (Direction::Left, origin.sub(term, Boundary::Grid, 1))
    } else {
        (Direction::Right, origin.add(term, Boundary::Grid, 1))
    };
    let found = term.search_next(&mut regex, origin, direction, Side::Left, None)?;
    let (a, b) = (
        grid_point(term, *found.start()),
        grid_point(term, *found.end()),
    );
    Some((a.min(b), a.max(b)))
}

fn escape(query: &str) -> String {
    let mut out = String::with_capacity(query.len());
    for c in query.chars() {
        if REGEX_META.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_characters_are_taken_literally() {
        assert_eq!(escape("a.b*(c)"), "a\\.b\\*\\(c\\)");
        assert_eq!(escape("plain words"), "plain words");
    }
}
