//! The file picker and viewer in the client: listing the focused pane's
//! checkout, opening a file, and what the viewer's keys ask for (an
//! agent, the clipboard, the editor).

use monarch_core::PaneId;
use monarch_proto::{ClientMsg, CodeFile};

use super::AttachApp;
use super::review::editor;
use crate::{FilePickStep, FilePicker, FileViewer, FileViewerStep, OverlayKey};

impl AttachApp {
    /// `prefix f`: asks the server for the focused pane's files and shows
    /// the picker, searchable while they arrive.
    pub(super) fn open_file_picker(&mut self) {
        let Some(pane) = self.state.focus() else {
            return self.say("no pane to open a file from");
        };
        let _ = self.link.send(&ClientMsg::ListFiles(pane));
        self.popups.files = Some(FilePicker::new(pane));
        self.focus.set_overlay(true);
        self.dirty = true;
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

    /// `monarch view` from `pane`'s shell: the viewer opens on `file`,
    /// in place of a file picker or viewer already open.
    pub(super) fn view_file(&mut self, pane: PaneId, file: CodeFile) {
        self.popups.files = None;
        let mut viewer = FileViewer::new(pane, file.path.clone());
        viewer.load(Ok(file));
        self.popups.viewer = Some(viewer);
        self.focus.set_overlay(true);
        self.dirty = true;
    }

    /// Keys while the viewer is open.
    pub(super) fn viewer_input(&mut self, bytes: &[u8]) {
        for key in OverlayKey::decode(bytes) {
            let Some(viewer) = self.popups.viewer.as_mut() else {
                break;
            };
            self.dirty = true;
            match viewer.key(key) {
                FileViewerStep::Open => {}
                FileViewerStep::Close => self.close_viewer(),
                FileViewerStep::Ask(quote) => return self.open_ask_with(Some(quote)),
                FileViewerStep::Copy(text) => self.copy(text),
                FileViewerStep::Edit(path) if std::path::Path::new(&path).is_absolute() => {
                    self.say("e edits files in the pane's checkout; this one is outside it");
                }
                FileViewerStep::Edit(path) => {
                    let pane = viewer.pane();
                    self.close_viewer();
                    let editor = editor(std::env::var("VISUAL").ok(), std::env::var("EDITOR").ok());
                    let _ = self.link.send(&ClientMsg::Edit { pane, path, editor });
                }
            }
        }
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
