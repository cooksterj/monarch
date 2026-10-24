//! What the prefix parser makes of host input.

/// One unit of parsed host input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    /// Bytes for the focused pane, exactly as the host sent them.
    Forward(Vec<u8>),
    /// The key byte pressed after the prefix (e.g. `b'%'`, `b'd'`).
    Command(u8),
}
