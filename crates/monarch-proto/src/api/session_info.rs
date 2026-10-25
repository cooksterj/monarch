//! `session.info`: what a running session holds, for `monarch ls`.

use serde::{Deserialize, Serialize};

/// A session's size and who is looking at it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Panes running in it.
    pub panes: u32,
    /// Its tabs.
    pub tabs: u32,
    /// Clients attached to it now.
    pub clients: u32,
    /// The server's process id.
    pub pid: u32,
}

impl SessionInfo {
    /// The method name; it takes no params.
    pub const METHOD: &'static str = "session.info";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let info = SessionInfo {
            panes: 3,
            tabs: 2,
            clients: 1,
            pid: 4242,
        };
        let json = serde_json::to_string(&info).expect("json");
        assert_eq!(serde_json::from_str::<SessionInfo>(&json).ok(), Some(info));
        assert!(
            serde_json::from_str::<SessionInfo>(r#"{"panes":1}"#).is_err(),
            "incomplete"
        );
    }
}
