//! The project picker: every worktree of every repository the session
//! works in, searched by project and branch.

use monarch_core::{Action, PaneId};
use monarch_proto::WorktreeEntry;

use super::{Picker, PickerItem, PickerStep, ProjectStep};
use crate::OverlayKey;

const TITLE: &str = "projects";
const KEYS: &str = " ↑↓ move · enter open · ctrl-x remove · esc close ";

/// The worktrees and the search over them; a removal waits for a second
/// Ctrl-x. Pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectPicker {
    picker: Picker,
    entries: Vec<WorktreeEntry>,
    /// The entry a first Ctrl-x picked, waiting for the second.
    confirm: Option<usize>,
}

impl Default for ProjectPicker {
    fn default() -> Self {
        Self {
            picker: Picker::new(TITLE, Vec::new()).with_text(KEYS, "reading worktrees…"),
            entries: Vec::new(),
            confirm: None,
        }
    }
}

impl ProjectPicker {
    /// Shows `entries`; `focus` is the focused pane, marked "here".
    pub fn load(&mut self, entries: Vec<WorktreeEntry>, focus: Option<PaneId>) {
        let items = entries.iter().map(|e| item(e, focus)).collect();
        self.picker = Picker::new(TITLE, items).with_text(KEYS, "no worktrees");
        self.entries = entries;
        self.confirm = None;
    }

    /// What is drawn.
    #[must_use]
    pub fn picker(&self) -> &Picker {
        &self.picker
    }

    /// Applies one key: type to search, ↑↓ move, Enter jumps to a pane
    /// there or opens a tab, Ctrl-x twice removes, Esc closes.
    pub fn key(&mut self, key: OverlayKey) -> ProjectStep {
        let chosen = self.picker.selected_index();
        let entry = chosen.and_then(|i| self.entries.get(i));
        let confirm = self.confirm.take();
        self.picker.set_title(TITLE);
        match (key, entry) {
            (OverlayKey::Enter, Some(e)) => match e.panes.first() {
                Some(pane) => ProjectStep::Focus(*pane),
                None => ProjectStep::OpenTab(e.path.clone()),
            },
            (OverlayKey::Ctrl('x'), Some(e)) => self.remove(e.clone(), chosen, confirm),
            (OverlayKey::Enter | OverlayKey::Ctrl('x'), None) => ProjectStep::Open,
            _ => match self.picker.key(key) {
                PickerStep::Close => ProjectStep::Close,
                _ => ProjectStep::Open,
            },
        }
    }

    /// Shows `text` as the title until the next key, where the eye already
    /// is: a refusal flashed on the status line is easy to miss.
    pub fn tell(&mut self, text: &str) {
        self.picker.set_title(text);
    }

    /// The first Ctrl-x asks, the second on the same row removes.
    fn remove(
        &mut self,
        e: WorktreeEntry,
        at: Option<usize>,
        confirm: Option<usize>,
    ) -> ProjectStep {
        if e.main {
            self.tell("the main checkout can't be removed");
            return ProjectStep::Open;
        }
        if !e.panes.is_empty() {
            let s = if e.panes.len() == 1 { "" } else { "s" };
            self.tell(&format!(
                "can't remove: close its {} pane{s} first",
                e.panes.len()
            ));
            return ProjectStep::Open;
        }
        if confirm.is_some() && confirm == at {
            return ProjectStep::Remove(e.path);
        }
        self.confirm = at;
        let name = e.path.rsplit('/').next().unwrap_or(&e.path);
        self.picker
            .set_title(format!("remove {name}? ctrl-x again"));
        ProjectStep::Open
    }
}

/// A row: `project  branch`, with what is there at the right.
fn item(entry: &WorktreeEntry, focus: Option<PaneId>) -> PickerItem {
    let branch = entry.branch.as_deref().unwrap_or("(detached)");
    let panes = match entry.panes.len() {
        _ if focus.is_some_and(|f| entry.panes.contains(&f)) => "here".to_owned(),
        0 => String::new(),
        1 => "1 pane".to_owned(),
        n => format!("{n} panes"),
    };
    let detail = match (entry.main, panes.is_empty()) {
        (true, true) => "main".to_owned(),
        (true, false) => format!("main · {panes}"),
        (false, _) => panes,
    };
    PickerItem {
        label: format!("{}  {branch}", entry.project),
        detail,
        // Never run: Enter is handled here, by entry.
        action: Action::Projects,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, branch: &str, main: bool, panes: &[u32]) -> WorktreeEntry {
        WorktreeEntry {
            project: "app".into(),
            path: path.into(),
            branch: Some(branch.into()),
            main,
            panes: panes.iter().map(|p| PaneId(*p)).collect(),
        }
    }

    fn loaded() -> ProjectPicker {
        let mut p = ProjectPicker::default();
        p.load(
            vec![
                entry("/src/app", "main", true, &[1, 2]),
                entry("/w/app/feat-x", "feat/x", false, &[]),
                entry("/w/app/busy", "busy", false, &[3]),
            ],
            Some(PaneId(3)),
        );
        p
    }

    #[test]
    fn rows_say_what_is_there() {
        let p = loaded();
        let rows: Vec<_> = p
            .picker()
            .rows()
            .map(|(i, _)| (i.label.as_str(), i.detail.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("app  main", "main · 2 panes"),
                ("app  feat/x", ""),
                ("app  busy", "here"),
            ]
        );
        assert_eq!(
            ProjectPicker::default().picker().text().map(|t| t.1),
            Some("reading worktrees…")
        );
    }

    #[test]
    fn enter_jumps_to_a_pane_or_opens_a_tab() {
        let mut p = loaded();
        assert_eq!(p.key(OverlayKey::Enter), ProjectStep::Focus(PaneId(1)));
        p.key(OverlayKey::Down);
        assert_eq!(
            p.key(OverlayKey::Enter),
            ProjectStep::OpenTab("/w/app/feat-x".into())
        );
        for c in "zzz".chars() {
            p.key(OverlayKey::Char(c));
        }
        assert_eq!(
            p.key(OverlayKey::Enter),
            ProjectStep::Open,
            "nothing matches"
        );
        assert_eq!(p.key(OverlayKey::Esc), ProjectStep::Close);
    }

    #[test]
    fn removal_needs_two_ctrl_x_on_a_free_worktree() {
        let mut p = loaded();
        assert_eq!(p.key(OverlayKey::Ctrl('x')), ProjectStep::Open);
        assert_eq!(p.picker().title(), "the main checkout can't be removed");
        p.key(OverlayKey::Down);
        assert_eq!(p.key(OverlayKey::Ctrl('x')), ProjectStep::Open);
        assert_eq!(p.picker().title(), "remove feat-x? ctrl-x again");
        p.key(OverlayKey::Down);
        p.key(OverlayKey::Up);
        assert_eq!(
            p.key(OverlayKey::Ctrl('x')),
            ProjectStep::Open,
            "moving cancels"
        );
        assert_eq!(
            p.key(OverlayKey::Ctrl('x')),
            ProjectStep::Remove("/w/app/feat-x".into())
        );
        p.key(OverlayKey::Down);
        assert_eq!(p.key(OverlayKey::Ctrl('x')), ProjectStep::Open);
        assert_eq!(p.picker().title(), "can't remove: close its 1 pane first");
        p.key(OverlayKey::Down);
        assert_eq!(p.picker().title(), "projects", "the next key clears it");
    }
}
