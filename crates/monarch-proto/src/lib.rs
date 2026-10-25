//! Monarch wire protocol: the engine-neutral screen model, the binary attach
//! stream (M1) and the public JSON-lines API (M3).
//!
//! The screen model lives here because it is produced by the server's
//! terminal engine, carried over the attach stream and rendered by the TUI;
//! defining it once keeps all three in agreement.

mod api;
mod attach;
mod error;
mod hello;
mod hello_ack;
mod review;
mod screen;

pub use api::{AgentReport, Request, Response, RpcError};
pub use attach::{
    AgentStatus, ClientMsg, Delta, FrameCodec, HostInfo, LayoutMsg, PaneLayout, PanePlace,
    PaneState, PaneSummary, RowCodec, RowDelta, Run, SelectShape, ServerMsg, Snapshot, StyleCache,
    StyleDef, StyleId, StyleTable, TabSummary,
};
pub use error::ProtoError;
pub use hello::Hello;
pub use hello_ack::HelloAck;
pub use review::{DiffLine, DiffLineKind, FileChange, Review, ReviewFile};

pub use screen::{
    Cell, CellAttrs, CellStyle, Cursor, CursorShape, GridPoint, ModeFlags, Rgb, Screen, ScreenSize,
    TermColor, TerminalModes,
};
