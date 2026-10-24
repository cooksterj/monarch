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
}
