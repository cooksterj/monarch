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
    ANIMATION_STEP, Applied, AttachApp, AttachOptions, AttachOutcome, AttachState, AttachView,
    CONFIG_POLL, ClickCounter, ClientEvent, ConfigWatch, ContextMenu, CopyMode, CopyStep,
    DRAG_STEP, DragScroll, FocusView, GlyphAnimator, HelpState, InputKeys, MULTI_CLICK,
    MouseAction, MouseRouter, PaneReplica, Picker, PickerItem, PickerStep, PopupViews,
    ProjectPicker, ProjectStep, ReviewState, ReviewStep, SelectEvent, SelectUnit, Selection,
    ServerLink, SessionEntry, SessionLister, SessionPicker, SessionStep, SharedFocus, WELCOME_STEP,
    WHEEL_LINES, WORKTREE_CHOICES, WelcomeState, WindowTitle, WorktreeForm, WorktreeStep,
};
pub use error::TuiError;
pub use host::{HostClipboard, HostModeMirror, HostProbe, HostReply};
pub use input::{
    HostFocus, HostInput, HostInputFilter, InputEvent, MOD_BITS, MouseButton, MouseEncoder,
    MouseEvent, MouseExtractor, MouseKind, OverlayKey, PrefixParser, WheelDirection,
};
pub use platform::{LocalClipboard, SystemClipboard};
pub use widgets::{
    Chrome, ContextMenuView, GlyphLook, HelpLine, HelpView, MouseOwner, PaneBorders, PaneView,
    PickerView, ReviewView, SelectionSpan, Sidebar, SidebarGroup, SidebarRow, StatusLine,
    StyleConverter, WelcomeView, WhichKey, WhichKeyEntry, WorktreeFormView,
};
