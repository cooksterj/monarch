//! Binds a style id to a style for the rest of a connection.

use serde::{Deserialize, Serialize};

use super::StyleId;
use crate::CellStyle;

/// "From now on, `id` means `style`." Sent before the first frame using it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleDef {
    /// The id being (re)defined.
    pub id: StyleId,
    /// Its style.
    pub style: CellStyle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_id_and_style() {
        let def = StyleDef {
            id: StyleId(3),
            style: CellStyle::default(),
        };
        assert_eq!(def.id, StyleId(3));
        assert!(def.style.is_plain());
    }
}
