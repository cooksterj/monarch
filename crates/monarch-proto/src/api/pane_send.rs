//! `pane.send`: type text into a pane as if the user had, optionally
//! pressing Enter after it. Agents use it to hand work to agents they
//! started.

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// Starts a bracketed paste.
const PASTE_START: &str = "\x1b[200~";
/// Ends one.
const PASTE_END: &str = "\x1b[201~";

/// The request. The answer is an empty object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneSend {
    /// The pane to type into.
    pub pane: PaneId,
    /// What to type.
    pub text: String,
    /// Press Enter afterwards (submits a prompt or runs a command).
    #[serde(default)]
    pub enter: bool,
}

impl PaneSend {
    /// The method name.
    pub const METHOD: &'static str = "pane.send";

    /// The Enter key.
    pub const ENTER: &'static [u8] = b"\r";

    /// The bytes that type the text into a program that has (`bracketed`)
    /// or hasn't asked for bracketed paste. Multi-line text is pasted
    /// when it may be, so its newlines don't submit early; paste markers
    /// inside it are dropped, so it can't end the paste and type keys.
    /// Otherwise each newline is typed as Enter, as a terminal pastes.
    #[must_use]
    pub fn typed(&self, bracketed: bool) -> Vec<u8> {
        let text = self.text.replace(PASTE_START, "").replace(PASTE_END, "");
        if bracketed && text.contains('\n') {
            format!("{PASTE_START}{text}{PASTE_END}").into_bytes()
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send(text: &str) -> PaneSend {
        PaneSend {
            pane: PaneId(3),
            text: text.into(),
            enter: false,
        }
    }

    #[test]
    fn round_trips_and_enter_is_optional() {
        let json = r#"{"pane":3,"text":"fix the tests"}"#;
        let parsed: PaneSend = serde_json::from_str(json).expect("json");
        assert_eq!(parsed, send("fix the tests"));
        let back = serde_json::to_string(&PaneSend {
            enter: true,
            ..parsed
        })
        .expect("json");
        assert!(back.contains(r#""enter":true"#), "{back}");
        assert!(
            serde_json::from_str::<PaneSend>(r#"{"pane":3}"#).is_err(),
            "text needed"
        );
    }

    #[test]
    fn one_line_is_typed_and_many_are_pasted_when_the_program_allows() {
        assert_eq!(send("ls").typed(true), b"ls");
        assert_eq!(send("a\nb").typed(true), b"\x1b[200~a\nb\x1b[201~");
        assert_eq!(
            send("a\nb\r\nc").typed(false),
            b"a\rb\rc",
            "newlines as Enter"
        );
    }

    #[test]
    fn text_cannot_end_the_paste_early() {
        let sneaky = send("a\n\x1b[201~rm -rf ~\r\x1b[200~b");
        assert_eq!(sneaky.typed(true), b"\x1b[200~a\nrm -rf ~\rb\x1b[201~");
        assert_eq!(send("").typed(true), b"");
    }
}
