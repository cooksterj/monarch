//! Strongly typed identifiers for tabs, panes and clients.
//!
//! All ids share one definition through [`define_id!`] so their behaviour
//! (display prefix, ordering, serde as a bare integer) cannot drift apart.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Defines a `u32` newtype id that displays as `<prefix><n>` (e.g. `p3`).
macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            /// The id that follows this one.
            #[must_use]
            pub fn next(self) -> Self {
                Self(self.0 + 1)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

define_id!(
    /// Identifies a tab within a session.
    TabId,
    "t"
);
define_id!(
    /// Identifies a pane within a session; stable for the pane's lifetime.
    PaneId,
    "p"
);
define_id!(
    /// Identifies an attached client connection.
    ClientId,
    "c"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_display_with_their_prefix() {
        assert_eq!(PaneId(3).to_string(), "p3");
        assert_eq!(TabId(1).to_string(), "t1");
        assert_eq!(ClientId(0).to_string(), "c0");
    }

    #[test]
    fn next_increments() {
        assert_eq!(PaneId(7).next(), PaneId(8));
    }

    #[test]
    fn serializes_as_bare_integer() {
        assert_eq!(serde_json::to_string(&PaneId(5)).ok(), Some("5".into()));
    }
}
