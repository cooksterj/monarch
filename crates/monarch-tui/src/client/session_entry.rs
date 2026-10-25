//! One running session, as the session picker lists it.

use monarch_proto::SessionInfo;

/// A session's name and, when its server says, what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEntry {
    /// The session's name.
    pub name: String,
    /// Panes, tabs and clients; `None` from a server too old to say.
    pub info: Option<SessionInfo>,
}

impl SessionEntry {
    /// `3 panes, 2 tabs`, or nothing when unknown.
    #[must_use]
    pub fn size(&self) -> String {
        let Some(info) = self.info else {
            return String::new();
        };
        let plural = |n: u32, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
        format!(
            "{}, {}",
            plural(info.panes, "pane"),
            plural(info.tabs, "tab")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_reads_naturally_and_is_empty_when_unknown() {
        let info = |panes, tabs| SessionInfo {
            panes,
            tabs,
            clients: 0,
            pid: 1,
        };
        let entry = |info| SessionEntry {
            name: "w".into(),
            info,
        };
        assert_eq!(entry(Some(info(3, 2))).size(), "3 panes, 2 tabs");
        assert_eq!(entry(Some(info(1, 1))).size(), "1 pane, 1 tab");
        assert_eq!(entry(None).size(), "");
    }
}
