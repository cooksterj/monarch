//! Monarch wire protocol: the engine-neutral screen model, the binary attach
//! stream (M1) and the public JSON-lines API (M2).
//!
//! The screen model lives here because it is produced by the server's
//! terminal engine, carried over the attach stream and rendered by the TUI;
//! defining it once keeps all three in agreement.

mod attach;
mod screen;

pub use attach::HostInfo;

pub use screen::{
    Cell, CellAttrs, CellStyle, Cursor, CursorShape, ModeFlags, Rgb, Screen, ScreenSize, TermColor,
    TerminalModes,
};
