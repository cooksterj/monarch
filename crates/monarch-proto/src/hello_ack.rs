//! The server's answer to a [`crate::Hello`].

use serde::{Deserialize, Serialize};

/// `{"ok":true,"server":"0.1.0"}`, or `ok: false` with a reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloAck {
    /// Whether the requested protocol follows.
    pub ok: bool,
    /// Server version.
    pub server: String,
    /// Why the hello was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The server's attach format ([`crate::Hello::WIRE`]); 0 from builds
    /// older than the field.
    #[serde(default)]
    pub wire: u32,
}

impl HelloAck {
    /// Acceptance from a server of `version`.
    #[must_use]
    pub fn accept(version: &str) -> Self {
        Self {
            ok: true,
            server: version.into(),
            error: None,
            wire: crate::Hello::WIRE,
        }
    }

    /// Refusal with a reason.
    #[must_use]
    pub fn refuse(version: &str, reason: impl Into<String>) -> Self {
        Self {
            ok: false,
            server: version.into(),
            error: Some(reason.into()),
            wire: crate::Hello::WIRE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_omits_the_error_field() {
        let line = serde_json::to_string(&HelloAck::accept("0.1.0")).expect("json");
        assert_eq!(line, r#"{"ok":true,"server":"0.1.0","wire":4}"#);
    }

    #[test]
    fn an_older_server_has_no_wire() {
        let old: HelloAck = serde_json::from_str(r#"{"ok":true,"server":"0.1.0"}"#).expect("parse");
        assert_eq!(old.wire, 0);
    }

    #[test]
    fn refusal_round_trips() {
        let ack = HelloAck::refuse("0.1.0", "json API not available yet");
        let back: HelloAck =
            serde_json::from_str(&serde_json::to_string(&ack).expect("json")).expect("parse");
        assert_eq!(back, ack);
        assert!(!back.ok);
    }
}
