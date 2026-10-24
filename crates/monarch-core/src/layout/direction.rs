//! A direction on screen, for focus moves and resizes.

use serde::{Deserialize, Serialize};

use super::SplitAxis;

/// Left, right, up or down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Towards column 0.
    Left,
    /// Towards the last column.
    Right,
    /// Towards row 0.
    Up,
    /// Towards the last row.
    Down,
}

impl Direction {
    /// Every direction.
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Up, Self::Down];

    /// The name used in config and action names.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
        }
    }

    /// The split axis whose divider moves in this direction.
    #[must_use]
    pub fn axis(self) -> SplitAxis {
        match self {
            Self::Left | Self::Right => SplitAxis::Columns,
            Self::Up | Self::Down => SplitAxis::Rows,
        }
    }

    /// Whether this direction points to larger coordinates.
    #[must_use]
    pub fn is_forward(self) -> bool {
        matches!(self, Self::Right | Self::Down)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_know_their_axis_and_sense() {
        assert_eq!(Direction::Left.axis(), SplitAxis::Columns);
        assert_eq!(Direction::Down.axis(), SplitAxis::Rows);
        assert!(Direction::Right.is_forward() && !Direction::Up.is_forward());
        assert_eq!(
            Direction::ALL.map(Direction::name),
            ["left", "right", "up", "down"]
        );
    }
}
