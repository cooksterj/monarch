//! The first line a client sends on a Monarch socket.

use serde::{Deserialize, Serialize};

/// Opening JSON line: which protocol the connection will speak.
/// `{"monarch":1,"proto":"attach","version":"0.1.0"}`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// Handshake format version (always 1 for now).
    pub monarch: u32,
    /// `"attach"` (binary frames follow) or `"json"` (JSON-RPC lines).
    pub proto: String,
    /// Client version.
    pub version: String,
    /// The attach stream's format ([`Hello::WIRE`]); 0 from builds older
    /// than the field.
    #[serde(default)]
    pub wire: u32,
}

impl Hello {
    /// The handshake format this build speaks.
    pub const FORMAT: u32 = 1;
    /// The attach stream's format. Bump it whenever an attach message
    /// changes shape: a client and server that differ refuse each other
    /// with a clear message instead of misreading frames.
    pub const WIRE: u32 = 1;
    /// Protocol name for the binary attach stream.
    pub const ATTACH: &'static str = "attach";
    /// Protocol name for the JSON-lines API.
    pub const JSON: &'static str = "json";

    /// An attach hello from this build.
    #[must_use]
    pub fn attach(version: &str) -> Self {
        Self {
            monarch: Self::FORMAT,
            proto: Self::ATTACH.into(),
            version: version.into(),
            wire: Self::WIRE,
        }
    }

    /// A JSON API hello from this build.
    #[must_use]
    pub fn json(version: &str) -> Self {
        Self {
            proto: Self::JSON.into(),
            ..Self::attach(version)
        }
    }

    /// Whether this asks for the JSON API in a format we speak.
    #[must_use]
    pub fn is_json(&self) -> bool {
        self.monarch == Self::FORMAT && self.proto == Self::JSON
    }

    /// What to tell someone attaching to `session` when its server speaks
    /// attach format `server` and the client `client`.
    #[must_use]
    pub fn wire_mismatch(session: &str, server: u32, client: u32) -> String {
        if server < client {
            format!(
                "session {session} runs an older build of Monarch than this one. Restart it \
                 to use this build: `pkill -f 'monarch server --daemon --session {session}$'`, \
                 then `monarch {session}` (its tabs, directories and Claude conversations \
                 come back)"
            )
        } else {
            format!(
                "session {session} runs a newer build of Monarch than this one: attach with \
                 the `monarch` that started it"
            )
        }
    }

    /// Whether this asks for the attach stream in a format we speak.
    #[must_use]
    pub fn is_attach(&self) -> bool {
        self.monarch == Self::FORMAT && self.proto == Self::ATTACH
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shape_matches_the_documented_line() {
        let line = serde_json::to_string(&Hello::attach("0.1.0")).expect("json");
        assert_eq!(
            line,
            r#"{"monarch":1,"proto":"attach","version":"0.1.0","wire":1}"#
        );
        let old: Hello =
            serde_json::from_str(r#"{"monarch":1,"proto":"attach","version":"0.1.0"}"#)
                .expect("parse");
        assert_eq!(old.wire, 0, "older builds send no wire");
    }

    #[test]
    fn a_mismatch_says_which_side_to_change() {
        let older = Hello::wire_mismatch("main", 0, 1);
        assert!(older.contains("pkill -f 'monarch server --daemon --session main$'"));
        assert!(older.contains("`monarch main`"));
        assert!(Hello::wire_mismatch("w", 2, 1).contains("newer build"));
    }

    #[test]
    fn only_known_format_and_proto_are_attach() {
        assert!(Hello::attach("x").is_attach());
        let json = Hello {
            proto: "json".into(),
            ..Hello::attach("x")
        };
        assert!(!json.is_attach());
        assert!(json.is_json() && Hello::json("x") == json);
        let future = Hello {
            monarch: 2,
            ..Hello::attach("x")
        };
        assert!(!future.is_attach());
    }
}
