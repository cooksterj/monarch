//! `pane.view`: open the file viewer over a pane, on a file named from
//! that pane's shell (`monarch view src/main.rs`).

use monarch_core::PaneId;
use serde::{Deserialize, Serialize};

/// The request; answered with an empty object once the attached clients
/// have been sent the file, or an error saying why it can't be shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewFile {
    /// The pane to open the viewer over (the caller's own).
    pub pane: PaneId,
    /// The file, absolute: the caller resolves it against its own working
    /// directory, which the server can't see.
    pub path: String,
}

impl ViewFile {
    /// The method name.
    pub const METHOD: &'static str = "pane.view";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_a_pane_and_a_path() {
        let view = ViewFile {
            pane: PaneId(3),
            path: "/w/a.rs".into(),
        };
        let json = serde_json::to_string(&view).expect("json");
        assert_eq!(serde_json::from_str::<ViewFile>(&json).ok(), Some(view));
        let pane = serde_json::to_string(&PaneId(3)).expect("json");
        let bare = format!(r#"{{"pane":{pane}}}"#);
        assert!(
            serde_json::from_str::<ViewFile>(&bare).is_err(),
            "needs a path"
        );
    }
}
