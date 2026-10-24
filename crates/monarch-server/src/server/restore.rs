//! Starting a session's panes: from its saved state after a crash, or
//! fresh from the config.

use crate::{PersistedSession, ServerConfig, ServerContext, ServerError};

/// Restores the saved session when asked and one exists, else starts
/// `config.panes`. A pane that cannot come back (its program is gone) is
/// skipped rather than failing the whole session.
pub(super) fn start_panes(ctx: &ServerContext, config: &ServerConfig) -> Result<(), ServerError> {
    if let Some(saved) = saved_session(config) {
        let ids: Vec<_> = saved
            .panes
            .iter()
            .map(|pane| {
                ctx.spawn_restored(pane)
                    .inspect_err(|e| eprintln!("monarch: not restoring a pane: {e}"))
                    .ok()
            })
            .collect();
        if ids.iter().any(Option::is_some) {
            if let Some(&Some(id)) = saved.focus.and_then(|i| ids.get(i)) {
                ctx.lock().set_focus(id);
            }
            return Ok(());
        }
    }
    for spec in &config.panes {
        ctx.spawn_pane(spec)?;
    }
    Ok(())
}

/// The session to restore, if restoring is on and the file holds one this
/// build understands. A damaged file starts a fresh session (and is
/// overwritten by it) instead of refusing to start.
fn saved_session(config: &ServerConfig) -> Option<PersistedSession> {
    if !config.restore {
        return None;
    }
    match config.state.as_ref()?.load() {
        Ok(Some(saved)) if saved.is_restorable() => Some(saved),
        Ok(_) => None,
        Err(e) => {
            eprintln!("monarch: {e}; starting a fresh session");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PersistedPane, StateFile};

    fn config(dir: &std::path::Path, restore: bool) -> ServerConfig {
        ServerConfig {
            state: Some(StateFile::new(dir.join("state.json"))),
            restore,
            ..ServerConfig::default()
        }
    }

    #[test]
    fn restores_only_when_asked_and_saved() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(saved_session(&config(dir.path(), true)), None, "no file");
        let session = PersistedSession::new(vec![PersistedPane::default()], Some(0));
        let file = config(dir.path(), true).state.expect("state");
        file.save(&session).expect("save");
        assert_eq!(saved_session(&config(dir.path(), true)), Some(session));
        assert_eq!(
            saved_session(&config(dir.path(), false)),
            None,
            "explicit panes win"
        );
    }

    #[test]
    fn a_damaged_file_starts_fresh() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("state.json"), b"garbage").expect("write");
        assert_eq!(saved_session(&config(dir.path(), true)), None);
    }
}
