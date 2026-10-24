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
    FocusView, MouseAction, MouseRouter, PaneReplica, ServerLink, SharedFocus, WHEEL_LINES,
    WindowTitle,
};
pub use error::TuiError;
pub use host::{HostModeMirror, HostProbe, HostReply};
pub use input::{
    HostFocus, HostInput, HostInputFilter, InputEvent, MOD_BITS, MouseButton, MouseEncoder,
    MouseEvent, MouseExtractor, MouseKind, PrefixParser, WheelDirection,
};
pub use widgets::{PaneView, StatusLine, StyleConverter};
