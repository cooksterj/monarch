//! Monarch terminal client: raw input, host mode mirroring and rendering.
//!
//! Rendering is pure: widgets take engine-neutral [`monarch_proto::Screen`]s
//! and draw them into a ratatui buffer.

mod client;
mod error;
mod host;
mod input;
mod widgets;

pub use client::{
    Applied, AttachApp, AttachOptions, AttachOutcome, AttachState, AttachView, ClientEvent,
    PaneReplica, ServerLink, SharedFocus, WindowTitle,
};
pub use error::TuiError;
pub use host::{HostModeMirror, HostProbe, HostReply};
pub use input::{HostFocus, HostInputFilter, InputEvent, PrefixParser};
pub use widgets::{PaneView, StatusLine, StyleConverter};
