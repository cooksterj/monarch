//! The host terminal: probing what it supports and mirroring the focused
//! pane's modes onto it (Terminal Fidelity Contract G2–G8).

mod host_mode_mirror;
mod host_probe;
mod host_reply;

pub use host_mode_mirror::HostModeMirror;
pub use host_probe::HostProbe;
pub use host_reply::HostReply;
