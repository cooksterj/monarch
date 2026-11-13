//! Entering and leaving an attach: the terminal is taken over (or reused
//! after a session switch) and given back unless another attach follows.

use std::sync::Arc;

use super::AttachApp;
use crate::client::{AttachOptions, AttachOutcome, ServerLink, SessionLister};
use crate::{HostTerminal, TuiError};

impl AttachApp {
    /// Connects, takes over the terminal (or reuses the one `host` holds)
    /// and runs until detach or exit. A switch to another session leaves
    /// the terminal in `host` for the next attach, so the host never sees
    /// the shell in between; any other ending gives it back.
    ///
    /// # Errors
    /// The server is unreachable or the terminal cannot be driven.
    pub fn attach(
        opts: &AttachOptions,
        lister: Arc<dyn SessionLister>,
        host: &mut HostTerminal,
    ) -> Result<AttachOutcome, TuiError> {
        // A held terminal is given back when the caller drops `host`.
        let (link, reader) = ServerLink::connect(&opts.socket)?;
        let ui = host
            .take_or_init()
            .map_err(|e| TuiError::io("taking over the terminal", e))?;
        let ended = Self::start(ui, link, reader, opts, lister).and_then(|mut app| {
            let outcome = app.run();
            app.finish()?;
            Ok((outcome?, app.ui))
        });
        match ended {
            Ok((outcome @ AttachOutcome::Switch(_), ui)) => {
                host.keep(ui);
                Ok(outcome)
            }
            ended => {
                ratatui::restore();
                ended.map(|(outcome, _)| outcome)
            }
        }
    }
}
