//! Server side of style interning: assigns ids to styles per connection.

use std::collections::HashMap;

use super::{StyleDef, StyleId};
use crate::CellStyle;

/// Default number of styles kept before the table starts over.
const DEFAULT_CAPACITY: usize = 4096;

/// Maps styles to ids for one connection, emitting a [`StyleDef`] the first
/// time a style is seen. The table only starts over between messages
/// ([`Self::begin_message`]), never in the middle of one, so every id inside
/// a message keeps a single meaning.
#[derive(Debug, Clone)]
pub struct StyleTable {
    ids: HashMap<CellStyle, StyleId>,
    capacity: usize,
}

impl Default for StyleTable {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }
}

impl StyleTable {
    /// An empty table that starts over once it holds `capacity` styles.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: HashMap::new(),
            capacity: capacity.max(1),
        }
    }

    /// Call before encoding each message. Returns `true` if the table was
    /// full and has been cleared: the client must then be told to clear its
    /// cache before this message's definitions.
    pub fn begin_message(&mut self) -> bool {
        let full = self.ids.len() >= self.capacity;
        if full {
            self.ids.clear();
        }
        full
    }

    /// Forgets everything (new connection or resync).
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// The id for `style`, appending a definition to `defs` if it is new.
    pub fn intern(&mut self, style: CellStyle, defs: &mut Vec<StyleDef>) -> StyleId {
        if style.is_plain() {
            return StyleId::DEFAULT;
        }
        let next = StyleId(u32::try_from(self.ids.len() + 1).unwrap_or(u32::MAX));
        *self.ids.entry(style).or_insert_with(|| {
            defs.push(StyleDef { id: next, style });
            next
        })
    }

    /// Number of styles currently interned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether no style is interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CellAttrs, TermColor};

    fn style(n: u8) -> CellStyle {
        CellStyle {
            fg: TermColor::Indexed(n),
            ..CellStyle::default()
        }
    }

    #[test]
    fn plain_style_is_id_zero_and_never_defined() {
        let mut table = StyleTable::default();
        let mut defs = Vec::new();
        assert_eq!(
            table.intern(CellStyle::default(), &mut defs),
            StyleId::DEFAULT
        );
        assert!(defs.is_empty());
    }

    #[test]
    fn styles_are_defined_once() {
        let mut table = StyleTable::default();
        let mut defs = Vec::new();
        let bold = CellStyle {
            attrs: CellAttrs::BOLD,
            ..CellStyle::default()
        };
        let a = table.intern(bold, &mut defs);
        let b = table.intern(style(1), &mut defs);
        assert_eq!(table.intern(bold, &mut defs), a);
        assert_ne!(a, b);
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0], StyleDef { id: a, style: bold });
    }

    #[test]
    fn a_full_table_starts_over_only_between_messages() {
        let mut table = StyleTable::with_capacity(2);
        let mut defs = Vec::new();
        assert!(!table.begin_message());
        for n in 1..=3 {
            table.intern(style(n), &mut defs);
        }
        assert_eq!(table.len(), 3, "never cleared mid-message");
        assert!(table.begin_message());
        assert!(table.is_empty());
        defs.clear();
        assert_eq!(table.intern(style(9), &mut defs), StyleId(1));
    }
}
