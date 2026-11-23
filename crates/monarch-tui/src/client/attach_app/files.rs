//! The file picker and viewer in the client: listing the focused pane's
//! checkout, opening a file, and what the viewer's keys ask for (an
//! agent, the clipboard, the editor).

use monarch_core::{Action, Key, PaneId};
use monarch_proto::{ClientMsg, CodeFile};

use super::review::editor;
use super::{AttachApp, AttachOutcome};
use crate::{FilePickStep, FilePicker, FileViewer, FileViewerStep, OverlayKey};

impl AttachApp {
    /// `prefix f`: views the file the mouse selection names, when there
    /// is one; else asks the server for the focused pane's files and shows
    /// the picker, searchable while they arrive.
    pub(super) fn open_file_picker(&mut self) {
        let Some(pane) = self.state.focus() else {
            return self.say("no pane to open a file from");
        };
        if let Some(selection) = self.selection.take().filter(|s| !s.is_empty()) {
            let (start, end) = selection.bounds();
            self.popups.viewing = Some(pane);
            let _ = self
                .link
                .send(&ClientMsg::ViewSelected { pane, start, end });
            self.dirty = true;
            return;
        }
        self.show_file_picker(FilePicker::new(pane));
    }

    /// Lists the picker's pane's files and shows it.
    fn show_file_picker(&mut self, picker: FilePicker) {
        let _ = self.link.send(&ClientMsg::ListFiles(picker.pane()));
        self.popups.files = Some(picker);
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// The selection `prefix f` was pressed on named no file: the picker
    /// opens searching for `text`.
    pub(super) fn not_a_file(&mut self, pane: PaneId, text: String) {
        if self.popups.viewing.take_if(|p| *p == pane).is_some() {
            self.show_file_picker(FilePicker::new(pane).with_query(text));
        }
    }

    /// The server's list of `pane`'s files, if the picker still waits.
    pub(super) fn listed_files(&mut self, pane: PaneId, files: Result<Vec<String>, String>) {
        let Some(picker) = self.popups.files.as_mut().filter(|p| p.pane() == pane) else {
            return;
        };
        match files {
            Ok(files) => picker.load(files),
            Err(why) => {
                self.close_file_picker();
                self.say(&why);
            }
        }
        self.dirty = true;
    }

    /// Keys while the picker is open.
    pub(super) fn file_picker_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(picker) = self.popups.files.as_mut() else {
                break;
            };
            self.dirty = true;
            match picker.key(key) {
                FilePickStep::Open => {}
                FilePickStep::Close => self.close_file_picker(),
                FilePickStep::Choose(path) => {
                    let pane = picker.pane();
                    self.close_file_picker();
                    self.open_viewer(pane, path);
                }
            }
        }
    }

    /// Opens the viewer on `path`, reading until the server answers.
    fn open_viewer(&mut self, pane: PaneId, path: String) {
        let msg = ClientMsg::OpenFile {
            pane,
            path: path.clone(),
        };
        let _ = self.link.send(&msg);
        self.popups.viewer = Some(FileViewer::new(pane, path));
        self.focus.set_overlay(true);
    }

    /// The server's answer for a file `pane`'s viewer waits on.
    pub(super) fn opened_file(&mut self, pane: PaneId, file: Result<CodeFile, String>) {
        let Some(viewer) = self.popups.viewer.as_mut().filter(|v| v.pane() == pane) else {
            return;
        };
        if file.as_ref().is_ok_and(|f| f.path != viewer.path()) {
            return; // the answer for a file viewed before this one
        }
        viewer.load(file);
        self.dirty = true;
    }

    /// `monarch view` from `pane`'s shell, or a selected name: the viewer
    /// opens on `file` (at `line`, when the name gave one), in place of a
    /// file picker or viewer already open.
    pub(super) fn view_file(&mut self, pane: PaneId, file: CodeFile, line: Option<usize>) {
        self.popups.files = None;
        self.popups.viewing = None;
        let mut viewer = FileViewer::new(pane, file.path.clone());
        viewer.load(Ok(file));
        if let Some(line) = line {
            viewer.jump(line);
        }
        self.popups.viewer = Some(viewer);
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Keys while the viewer is open. The prefix works here too: `prefix
    /// e` asks about the viewer's lines, the prefix twice is the viewer's
    /// own Ctrl-b (page up), and any other command closes the viewer first.
    pub(super) fn viewer_input(&mut self, bytes: &[u8]) -> Option<AttachOutcome> {
        for key in OverlayKey::decode(bytes) {
            let is_prefix = key == OverlayKey::Ctrl(char::from(self.opts.prefix + 0x60));
            if self.prefix_pending && !is_prefix {
                self.on_prefix(false);
                return self.viewer_command(key);
            }
            if is_prefix && !self.prefix_pending {
                self.on_prefix(true);
                continue;
            }
            self.on_prefix(false);
            let Some(viewer) = self.popups.viewer.as_mut() else {
                break;
            };
            self.dirty = true;
            match viewer.key(key) {
                FileViewerStep::Open => {}
                FileViewerStep::Close => self.close_viewer(),
                FileViewerStep::Ask(quote) => {
                    self.open_ask_with(Some(quote));
                    return None;
                }
                FileViewerStep::Copy(text) => self.copy(text),
                FileViewerStep::Edit { path, .. } if std::path::Path::new(&path).is_absolute() => {
                    self.say("e edits files in the pane's checkout; this one is outside it");
                }
                FileViewerStep::Edit { path, line } => {
                    let pane = viewer.pane();
                    self.close_viewer();
                    self.edit_at(pane, path, line);
                }
            }
        }
        None
    }

    /// Opens `path` from `pane`'s checkout in the user's editor at `line`.
    fn edit_at(&mut self, pane: PaneId, path: String, line: usize) {
        let editor = editor(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok());
        let msg = ClientMsg::Edit {
            pane,
            path,
            editor,
            line: Some(line),
        };
        let _ = self.link.send(&msg);
    }

    /// The key after the prefix in the viewer: the ask quotes the viewer's
    /// lines; other commands run with the viewer closed.
    fn viewer_command(&mut self, key: OverlayKey) -> Option<AttachOutcome> {
        let OverlayKey::Char(c) = key else {
            return None;
        };
        let action =
            Key::from_byte(u8::try_from(c).ok()?).and_then(|k| self.opts.keymap.action(k))?;
        if action == Action::Ask {
            let quote = self.popups.viewer.as_ref().and_then(FileViewer::quote);
            self.open_ask_with(quote);
            return None;
        }
        self.close_viewer();
        self.run_action(action)
    }

    fn close_file_picker(&mut self) {
        self.popups.files = None;
        self.focus.set_overlay(self.any_overlay());
    }

    fn close_viewer(&mut self) {
        self.popups.viewer = None;
        self.focus.set_overlay(self.any_overlay());
    }
}
