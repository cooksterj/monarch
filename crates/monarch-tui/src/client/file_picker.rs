//! The file picker: every file in the focused pane's checkout, searched
//! by path, to open in the file viewer.

use monarch_core::{Action, PaneId};

use super::{FilePickStep, Picker, PickerItem, PickerStep};
use crate::OverlayKey;

const TITLE: &str = "open file";
const KEYS: &str = " type to search · enter view · esc close ";

/// The checkout's files and the search over them. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePicker {
    pane: PaneId,
    picker: Picker,
    paths: Vec<String>,
}

impl FilePicker {
    /// A picker for `pane`'s checkout, waiting for its files.
    #[must_use]
    pub fn new(pane: PaneId) -> Self {
        Self {
            pane,
            picker: Picker::new(TITLE, Vec::new()).with_text(KEYS, "reading files…"),
            paths: Vec::new(),
        }
    }

    /// The same picker with `query` already typed.
    #[must_use]
    pub fn with_query(self, query: impl Into<String>) -> Self {
        Self {
            picker: self.picker.with_query(query),
            ..self
        }
    }

    /// The pane whose checkout is listed.
    #[must_use]
    pub fn pane(&self) -> PaneId {
        self.pane
    }

    /// Shows `paths`, keeping anything already typed.
    pub fn load(&mut self, paths: Vec<String>) {
        let items = paths
            .iter()
            .map(|path| PickerItem {
                label: path.clone(),
                detail: String::new(),
                action: Action::OpenFile,
            })
            .collect();
        let query = self.picker.query().to_owned();
        self.picker = Picker::new(TITLE, items)
            .with_text(KEYS, "no file matches")
            .with_query(query);
        self.paths = paths;
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// One key: type to search, ↑↓ move, Enter views the file, Esc closes.
    pub fn key(&mut self, key: OverlayKey) -> FilePickStep {
        match self.picker.key(key) {
            PickerStep::Run(_) => self
                .picker
                .selected_index()
                .and_then(|i| self.paths.get(i))
                .map_or(FilePickStep::Open, |p| FilePickStep::Choose(p.clone())),
            PickerStep::Close => FilePickStep::Close,
            PickerStep::Open | PickerStep::Answer(..) => FilePickStep::Open,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(p: &mut FilePicker, text: &str) {
        for c in text.chars() {
            assert_eq!(p.key(OverlayKey::Char(c)), FilePickStep::Open);
        }
    }

    #[test]
    fn search_then_enter_chooses_the_best_match() {
        let mut p = FilePicker::new(PaneId(2));
        assert_eq!(p.pane(), PaneId(2));
        assert_eq!(
            FilePicker::new(PaneId(2))
                .with_query("a.rs")
                .picker()
                .query(),
            "a.rs"
        );
        typed(&mut p, "pio");
        assert_eq!(
            p.key(OverlayKey::Enter),
            FilePickStep::Open,
            "still loading"
        );
        p.load(vec![
            "README.md".into(),
            "crates/server/src/pane_io.rs".into(),
        ]);
        assert_eq!(p.picker().query(), "pio", "typing survives the load");
        assert_eq!(
            p.key(OverlayKey::Enter),
            FilePickStep::Choose("crates/server/src/pane_io.rs".into())
        );
        assert_eq!(p.key(OverlayKey::Esc), FilePickStep::Close);
    }
}
