//! The inputs to [`super::PaneRuntime::spawn`].

use std::sync::Arc;

use monarch_core::PaneId;
use monarch_proto::ScreenSize;
use monarch_term::HostProfile;
use tokio::sync::Notify;
use tokio::sync::mpsc::UnboundedSender;

use crate::{PaneEnv, PaneEvent, PaneSpec};

/// Everything needed to start a pane.
pub struct SpawnParams<'a> {
    /// Its id.
    pub id: PaneId,
    /// What to run.
    pub spec: &'a PaneSpec,
    /// Session environment.
    pub env: &'a PaneEnv,
    /// Initial size.
    pub size: ScreenSize,
    /// Host colours and cell size for query replies.
    pub profile: HostProfile,
    /// Lines of scrollback.
    pub scrollback: usize,
    /// Where pane events go.
    pub events: UnboundedSender<PaneEvent>,
    /// Woken when the pane gets new output.
    pub wake: Arc<Notify>,
}
