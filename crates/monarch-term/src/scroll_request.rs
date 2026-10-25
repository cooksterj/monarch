//! A request to move the viewport through scrollback.

/// How to scroll a pane's viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollRequest {
    /// Positive scrolls up into history, negative back towards live output.
    Lines(i32),
    /// One screen up.
    PageUp,
    /// One screen down.
    PageDown,
    /// Oldest history.
    Top,
    /// Live output.
    Bottom,
    /// Show this absolute line (0 = oldest scrollback line) at the top,
    /// as near as history allows. Repeating it changes nothing, so a
    /// client can send it again before the last one has taken effect.
    TopLine(i64),
}
