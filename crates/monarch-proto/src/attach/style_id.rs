//! Per-connection identifier for an interned cell style.

use serde::{Deserialize, Serialize};

/// Index into a connection's style table. Id 0 is always the default style
/// and is never defined on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StyleId(pub u32);

impl StyleId {
    /// The default (plain) style.
    pub const DEFAULT: Self = Self(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(StyleId::default(), StyleId::DEFAULT);
        assert_eq!(StyleId::DEFAULT.0, 0);
    }
}
