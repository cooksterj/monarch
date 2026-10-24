//! Monarch terminal client: raw input, host mode mirroring and rendering.
//!
//! Rendering is pure: widgets take engine-neutral [`monarch_proto::Screen`]s
//! and draw them into a ratatui buffer.

mod client;
mod error;
mod host;
mod input;
mod platform;
mod widgets;

pub use client::{
    Applied, AttachApp, AttachOptions, AttachOutcome, AttachState, AttachView, ClickCounter,
    ClientEvent, FocusView, GridPoint, MULTI_CLICK, MouseAction, MouseRouter, PaneReplica, Picker,
    PickerItem, PickerStep, SelectEvent, SelectUnit, Selection, ServerLink, SharedFocus,
    WHEEL_LINES, WindowTitle,
};
pub use error::TuiError;
pub use host::{HostClipboard, HostModeMirror, HostProbe, HostReply};
pub use input::{
    HostFocus, HostInput, HostInputFilter, InputEvent, MOD_BITS, MouseButton, MouseEncoder,
    MouseEvent, MouseExtractor, MouseKind, OverlayKey, PrefixParser, WheelDirection,
};
pub use platform::{LocalClipboard, SystemClipboard};
pub use widgets::{
    PaneBorders, PaneView, PickerView, SelectionSpan, StatusLine, StyleConverter, WhichKey,
    WhichKeyEntry,
};
