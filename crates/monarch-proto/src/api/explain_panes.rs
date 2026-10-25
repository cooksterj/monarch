//! `agent.explain`: why Monarch thinks a pane's agent is in its state.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// Which pane to explain; every pane when none is named. The result is a
/// list of [`crate::PaneExplanation`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainPanes {
    /// The pane, or `None` for all of them.
    #[serde(default)]
    pub pane: Option<PaneId>,
}

impl ExplainPanes {
    /// The method name.
    pub const METHOD: &'static str = "agent.explain";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pane_is_optional() {
        let all: ExplainPanes = serde_json::from_str("{}").expect("json");
        assert_eq!(all.pane, None);
        let one = ExplainPanes {
            pane: Some(PaneId(3)),
        };
        let json = serde_json::to_string(&one).expect("json");
        assert_eq!(serde_json::from_str::<ExplainPanes>(&json).ok(), Some(one));
        assert!(serde_json::from_str::<ExplainPanes>(r#"{"pane":"x"}"#).is_err());
    }
}
