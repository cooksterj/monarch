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
    ANIMATION_STEP, AgentInfo, Applied, AttachApp, AttachOptions, AttachOutcome, AttachState,
    AttachView, AttentionItem, CONFIG_POLL, ClickCounter, ClientEvent, ConfigWatch, ContextMenu,
    CopyMode, CopyStep, DRAG_STEP, DividerGrab, DragScroll, FocusView, GlyphAnimator, GoToPane,
    GoToPicker, HelpState, InfoStep, InputKeys, LaunchPicker, LaunchStep, MAX_EXCERPT, MULTI_CLICK,
    MouseAction, MouseRouter, NamePrompt, NameStep, NameTarget, NotePrompt, NoteStep, PaneReplica,
    Picker, PickerItem, PickerStep, PopupViews, ProjectPicker, ProjectStep, QuickAnswer,
    ReviewNote, ReviewState, ReviewStep, SelectEvent, SelectUnit, Selection, ServerLink,
    SessionEntry, SessionLister, SessionPicker, SessionStep, SharedFocus, WELCOME_STEP,
    WHEEL_LINES, WORKTREE_CHOICES, WelcomeState, WindowTitle, WorktreeForm, WorktreeStep,
};
pub use error::TuiError;
pub use host::{HostBackend, HostClipboard, HostModeMirror, HostProbe, HostReply, HostTerminal};
pub use input::{
    HostFocus, HostInput, HostInputFilter, InputEvent, MOD_BITS, MouseButton, MouseEncoder,
    MouseEvent, MouseExtractor, MouseKind, OverlayKey, PrefixParser, WheelDirection,
};
pub use platform::{LocalClipboard, SystemClipboard};
pub use widgets::{
    AgentInfoView, Chrome, ContextMenuView, GlyphLook, HelpLine, HelpView, MouseOwner, PageView,
    PaneBorders, PaneView, PickerView, ReviewView, SelectionSpan, Sidebar, SidebarGroup,
    SidebarRow, StatusLine, StyleConverter, SubagentRow, UnderlineStyle, WelcomeView, WhichKey,
    WhichKeyEntry, WorktreeFormView,
};
