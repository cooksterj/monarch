//! `tab.rename`: name a tab, or give it its automatic name back.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// The request; answered with a [`crate::TabRenamed`]. The tab is the one
/// numbered `tab`, else the one holding `pane`, else the one shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabRename {
    /// The tab's number, counting from 1 as the tab bar does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<usize>,
    /// A pane in the tab (the caller's own, from `MONARCH_PANE`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<PaneId>,
    /// The new name; blank gives the automatic one back.
    pub name: String,
}

impl TabRename {
    /// The method name.
    pub const METHOD: &'static str = "tab.rename";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_name_is_required() {
        let bare: TabRename = serde_json::from_str(r#"{"name":"api"}"#).expect("parse");
        assert_eq!((bare.tab, bare.pane), (None, None));
        let json = serde_json::to_string(&TabRename {
            tab: Some(2),
            pane: None,
            name: String::new(),
        })
        .expect("json");
        assert_eq!(json, r#"{"tab":2,"name":""}"#);
        assert!(
            serde_json::from_str::<TabRename>(r#"{"tab":1}"#).is_err(),
            "needs a name"
        );
    }
}
