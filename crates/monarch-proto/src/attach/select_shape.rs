//! How a range of grid cells is turned into copied text.

use serde::{Deserialize, Serialize};

/// The shape of a selection between two grid points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SelectShape {
    /// Running text from the first point to the last, like a drag; lines
    /// the terminal wrapped come back joined.
    #[default]
    Chars,
    /// Whole lines from the first point's line to the last's.
    Lines,
    /// The rectangle with the two points at opposite corners.
    Block,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_text_is_the_default() {
        assert_eq!(SelectShape::default(), SelectShape::Chars);
        let bytes = postcard::to_allocvec(&SelectShape::Block).expect("encode");
        assert_eq!(
            postcard::from_bytes::<SelectShape>(&bytes).ok(),
            Some(SelectShape::Block)
        );
    }
}
