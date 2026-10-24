//! How far a selection reaches from where the mouse is.

/// One click selects cells, two select words, three select lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectUnit {
    /// Cell by cell.
    #[default]
    Cell,
    /// Whole words.
    Word,
    /// Whole lines.
    Line,
}

impl SelectUnit {
    /// The unit for a click count (1, 2, 3, then cycling).
    #[must_use]
    pub fn from_clicks(clicks: u8) -> Self {
        match clicks.saturating_sub(1) % 3 {
            0 => Self::Cell,
            1 => Self::Word,
            _ => Self::Line,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicks_cycle_through_units() {
        assert_eq!(SelectUnit::from_clicks(1), SelectUnit::Cell);
        assert_eq!(SelectUnit::from_clicks(2), SelectUnit::Word);
        assert_eq!(SelectUnit::from_clicks(3), SelectUnit::Line);
        assert_eq!(SelectUnit::from_clicks(4), SelectUnit::Cell);
        assert_eq!(SelectUnit::from_clicks(0), SelectUnit::Cell);
    }
}
