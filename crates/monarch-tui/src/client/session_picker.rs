//! The session picker: every running session, to switch to, or a new name
//! to start one.

use monarch_core::{Action, is_session_name};

use super::{Picker, PickerItem, PickerStep, SessionEntry, SessionStep};
use crate::OverlayKey;

const TITLE: &str = "sessions";
const KEYS: &str = " ↑↓ move · enter switch · new name + enter starts one · esc close ";
const NO_MATCH: &str = "no match: enter starts a new session by this name";

/// The sessions and the search over them. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPicker {
    picker: Picker,
    entries: Vec<SessionEntry>,
    /// The session this client is attached to.
    current: String,
    /// The list has arrived (until then Enter does nothing).
    loaded: bool,
}

impl SessionPicker {
    /// A picker waiting for the list, opened from session `current`.
    #[must_use]
    pub fn new(current: impl Into<String>) -> Self {
        Self {
            picker: Picker::new(TITLE, Vec::new()).with_text(KEYS, "reading sessions…"),
            entries: Vec::new(),
            current: current.into(),
            loaded: false,
        }
    }

    /// Shows `entries`, the current session marked `here`.
    pub fn load(&mut self, entries: Vec<SessionEntry>) {
        let items = entries.iter().map(|e| item(e, &self.current)).collect();
        self.picker = Picker::new(TITLE, items).with_text(KEYS, NO_MATCH);
        self.entries = entries;
        self.loaded = true;
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Applies one key: type to search, ↑↓ move, Enter switches to the
    /// highlighted session (or starts one named by the query when none
    /// matches), Esc closes.
    pub fn key(&mut self, key: OverlayKey) -> SessionStep {
        self.picker.set_title(TITLE);
        if key != OverlayKey::Enter {
            return match self.picker.key(key) {
                PickerStep::Close => SessionStep::Close,
                _ => SessionStep::Open,
            };
        }
        if !self.loaded {
            return SessionStep::Open;
        }
        let chosen = self
            .picker
            .selected_index()
            .and_then(|i| self.entries.get(i));
        let name = match chosen {
            Some(entry) => entry.name.clone(),
            None => self.picker.query().trim().to_owned(),
        };
        if name == self.current {
            SessionStep::Close
        } else if is_session_name(&name) {
            SessionStep::Switch(name)
        } else {
            self.picker.set_title("a name: letters, digits, - _ .");
            SessionStep::Open
        }
    }
}

/// A row: the name, with its size and who is there at the right.
fn item(entry: &SessionEntry, current: &str) -> PickerItem {
    let here = entry.name == current;
    let attached = !here && entry.info.is_some_and(|i| i.clients > 0);
    let size = entry.size();
    let notes = [
        size.as_str(),
        if attached { "attached" } else { "" },
        if here { "here" } else { "" },
    ];
    let detail: Vec<&str> = notes.into_iter().filter(|n| !n.is_empty()).collect();
    PickerItem {
        label: entry.name.clone(),
        detail: detail.join(" · "),
        // Never run: Enter is handled here, by entry.
        action: Action::Sessions,
    }
}

#[cfg(test)]
mod tests {
    use monarch_proto::SessionInfo;

    use super::*;

    fn entry(name: &str, clients: u32) -> SessionEntry {
        SessionEntry {
            name: name.into(),
            info: Some(SessionInfo {
                panes: 2,
                tabs: 1,
                clients,
                pid: 1,
            }),
        }
    }

    fn loaded() -> SessionPicker {
        let mut p = SessionPicker::new("main");
        p.load(vec![entry("main", 1), entry("work", 1), entry("old", 0)]);
        p
    }

    fn typed(p: &mut SessionPicker, text: &str) {
        text.chars().for_each(|c| {
            p.key(OverlayKey::Char(c));
        });
    }

    #[test]
    fn rows_say_size_and_who_is_there() {
        let p = loaded();
        let rows: Vec<_> = p
            .picker()
            .rows()
            .map(|(i, _)| (i.label.as_str(), i.detail.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("main", "2 panes, 1 tab · here"),
                ("work", "2 panes, 1 tab · attached"),
                ("old", "2 panes, 1 tab"),
            ]
        );
    }

    #[test]
    fn enter_switches_or_closes_on_the_current_one() {
        let mut p = loaded();
        assert_eq!(p.key(OverlayKey::Enter), SessionStep::Close, "already here");
        p.key(OverlayKey::Down);
        assert_eq!(p.key(OverlayKey::Enter), SessionStep::Switch("work".into()));
        assert_eq!(p.key(OverlayKey::Esc), SessionStep::Close);
    }

    #[test]
    fn a_new_name_starts_a_session_and_a_bad_one_says_why() {
        let mut p = loaded();
        typed(&mut p, "zz9");
        assert_eq!(p.key(OverlayKey::Enter), SessionStep::Switch("zz9".into()));
        p.key(OverlayKey::Ctrl('u'));
        typed(&mut p, "a/b");
        assert_eq!(p.key(OverlayKey::Enter), SessionStep::Open);
        assert_eq!(p.picker().title(), "a name: letters, digits, - _ .");
    }

    #[test]
    fn enter_waits_for_the_list() {
        let mut p = SessionPicker::new("main");
        typed(&mut p, "new");
        assert_eq!(p.key(OverlayKey::Enter), SessionStep::Open);
        assert_eq!(p.picker().text().map(|t| t.1), Some("reading sessions…"));
    }
}
