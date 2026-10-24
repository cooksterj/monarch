//! Client side of style interning: resolves ids back to styles.

use super::{StyleDef, StyleId};
use crate::CellStyle;

/// Highest id a client accepts; anything above is ignored rather than
/// letting a bad frame allocate without bound.
const MAX_ID: usize = 1 << 20;

/// The client's copy of a connection's style table.
#[derive(Debug, Clone, Default)]
pub struct StyleCache {
    styles: Vec<CellStyle>,
}

impl StyleCache {
    /// An empty cache (only the default style is known).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a definition.
    pub fn define(&mut self, def: StyleDef) {
        let Ok(index) = usize::try_from(def.id.0) else {
            return;
        };
        if index == 0 || index > MAX_ID {
            return;
        }
        if self.styles.len() <= index {
            self.styles.resize(index + 1, CellStyle::default());
        }
        self.styles[index] = def.style;
    }

    /// The style for `id`; unknown ids resolve to the default style.
    #[must_use]
    pub fn get(&self, id: StyleId) -> CellStyle {
        usize::try_from(id.0)
            .ok()
            .and_then(|i| self.styles.get(i))
            .copied()
            .unwrap_or_default()
    }

    /// Forgets every definition.
    pub fn clear(&mut self) {
        self.styles.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CellAttrs;

    fn bold() -> CellStyle {
        CellStyle {
            attrs: CellAttrs::BOLD,
            ..CellStyle::default()
        }
    }

    #[test]
    fn defined_ids_resolve() {
        let mut cache = StyleCache::new();
        cache.define(StyleDef {
            id: StyleId(2),
            style: bold(),
        });
        assert_eq!(cache.get(StyleId(2)), bold());
        assert!(cache.get(StyleId(1)).is_plain());
    }

    #[test]
    fn unknown_zero_and_absurd_ids_are_harmless() {
        let mut cache = StyleCache::new();
        cache.define(StyleDef {
            id: StyleId(0),
            style: bold(),
        });
        cache.define(StyleDef {
            id: StyleId(u32::MAX),
            style: bold(),
        });
        assert!(cache.get(StyleId::DEFAULT).is_plain());
        assert!(cache.get(StyleId(99)).is_plain());
    }

    #[test]
    fn clear_forgets_definitions() {
        let mut cache = StyleCache::new();
        cache.define(StyleDef {
            id: StyleId(1),
            style: bold(),
        });
        cache.clear();
        assert!(cache.get(StyleId(1)).is_plain());
    }
}
