//! What the prefix parser makes of host input.

use monarch_core::Key;

/// One unit of parsed host input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    /// Bytes for the focused pane, exactly as the host sent them.
    Forward(Vec<u8>),
    /// The key pressed after the prefix.
    Command(Key),
}
