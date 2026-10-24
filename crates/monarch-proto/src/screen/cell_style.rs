//! Everything about a cell's appearance except its text.

use serde::{Deserialize, Serialize};

use super::{CellAttrs, TermColor};

/// Colours and attributes of a cell. Small and `Copy` so runs of equal
/// style can be detected cheaply when building deltas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CellStyle {
    /// Foreground colour.
    pub fg: TermColor,
    /// Background colour.
    pub bg: TermColor,
    /// Underline colour; `None` follows the foreground.
    pub underline: Option<TermColor>,
    /// Text attributes.
    pub attrs: CellAttrs,
}

impl CellStyle {
    /// Whether this is the terminal's plain default style.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_detection() {
        assert!(CellStyle::default().is_plain());
        let bold = CellStyle {
            attrs: CellAttrs::BOLD,
            ..CellStyle::default()
        };
        assert!(!bold.is_plain());
    }
}
