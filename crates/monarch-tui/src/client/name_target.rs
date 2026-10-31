//! What a rename prompt names: a tab or a single pane.

use monarch_core::PaneId;
use monarch_proto::ClientMsg;

/// The thing being named, by a pane (a tab by any pane in it, so a tab
/// closing meanwhile can't get another tab renamed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameTarget {
    /// The tab holding this pane.
    Tab(PaneId),
    /// This pane.
    Pane(PaneId),
}

impl NameTarget {
    /// The message asking the server to give it `name`.
    #[must_use]
    pub fn rename(self, name: String) -> ClientMsg {
        match self {
            Self::Tab(pane) => ClientMsg::RenameTab { pane, name },
            Self::Pane(pane) => ClientMsg::RenamePane { pane, name },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_target_asks_for_its_own_rename() {
        let tab = NameTarget::Tab(PaneId(1)).rename("a".into());
        assert_eq!(
            tab,
            ClientMsg::RenameTab {
                pane: PaneId(1),
                name: "a".into()
            }
        );
        let pane = NameTarget::Pane(PaneId(2)).rename(String::new());
        assert_eq!(
            pane,
            ClientMsg::RenamePane {
                pane: PaneId(2),
                name: String::new()
            }
        );
    }
}
