//! The answer to `tab.rename`: which tab, and what it is called now.

use serde::{Deserialize, Serialize};

/// The tab renamed and its name after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabRenamed {
    /// Its number, counting from 1.
    pub tab: usize,
    /// The name it was given; `None` when it is back to the automatic one.
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cleared_name_is_null() {
        let cleared = TabRenamed { tab: 1, name: None };
        let json = serde_json::to_string(&cleared).expect("json");
        assert_eq!(json, r#"{"tab":1,"name":null}"#);
        assert_eq!(
            serde_json::from_str::<TabRenamed>(&json).ok(),
            Some(cleared)
        );
    }
}
