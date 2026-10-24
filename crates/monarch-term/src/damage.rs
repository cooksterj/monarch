//! Which viewport rows changed since the last frame.

/// Rows to redraw. Lets the frame coalescer copy only what changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Damage {
    /// Everything (scroll, resize, mode switch).
    Full,
    /// These viewport rows, ascending, without duplicates.
    Rows(Vec<u16>),
}

impl Damage {
    /// Nothing changed.
    #[must_use]
    pub fn none() -> Self {
        Self::Rows(Vec::new())
    }

    /// Whether anything needs redrawing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Rows(rows) if rows.is_empty())
    }

    /// Combines damage from two frames.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Full, _) | (_, Self::Full) => Self::Full,
            (Self::Rows(mut a), Self::Rows(b)) => {
                a.extend(b);
                a.sort_unstable();
                a.dedup();
                Self::Rows(a)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_unions_rows() {
        let d = Damage::Rows(vec![3, 1]).merge(Damage::Rows(vec![1, 2]));
        assert_eq!(d, Damage::Rows(vec![1, 2, 3]));
    }

    #[test]
    fn full_absorbs_everything() {
        assert_eq!(Damage::none().merge(Damage::Full), Damage::Full);
        assert!(Damage::none().is_empty());
        assert!(!Damage::Full.is_empty());
    }
}
