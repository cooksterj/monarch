//! One change of a pane's agent status, as `agent.explain` lists them.

use monarch_core::{AgentState, UnixMillis};
use serde::{Deserialize, Serialize};

use crate::StatusSource;

/// A state the pane's agent entered: which, from what signal, with what
/// activity, and when.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentTransition {
    /// The state entered.
    pub state: AgentState,
    /// The signal that put it there.
    pub source: StatusSource,
    /// The activity line that came with it, if any.
    pub activity: Option<String>,
    /// When.
    pub at: UnixMillis,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json_and_the_attach_encoding() {
        let transition = AgentTransition {
            state: AgentState::Blocked,
            source: StatusSource::Hooks,
            activity: Some("Allow Bash?".into()),
            at: UnixMillis(1_000),
        };
        let json = serde_json::to_string(&transition).expect("json");
        assert_eq!(
            serde_json::from_str::<AgentTransition>(&json).ok(),
            Some(transition.clone())
        );
        let bytes = postcard::to_allocvec(&transition).expect("encode");
        assert_eq!(
            postcard::from_bytes::<AgentTransition>(&bytes).ok(),
            Some(transition)
        );
        assert!(serde_json::from_str::<AgentTransition>(r#"{"state":"blocked"}"#).is_err());
    }
}
