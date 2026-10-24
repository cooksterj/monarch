//! Operating-system specifics, behind traits so callers stay portable.

#[cfg(not(target_os = "macos"))]
mod linux;
mod local_clipboard;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(not(target_os = "macos"))]
pub use linux::SystemClipboard;
pub use local_clipboard::LocalClipboard;
#[cfg(target_os = "macos")]
pub use macos::SystemClipboard;
