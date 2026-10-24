//! Monarch terminal client: raw input, host mode mirroring and rendering.
//!
//! Rendering is pure: widgets take engine-neutral [`monarch_proto::Screen`]s
//! and draw them into a ratatui buffer.

mod input;
mod widgets;

pub use input::{InputEvent, PrefixParser};
pub use widgets::{PaneView, StyleConverter};
