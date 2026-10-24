//! The session server process: socket, panes and background tasks.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::{
    AttachConnection, CommandWatcher, EventLoop, FrameCoalescer, PaneEnv, Persister, ServerConfig,
    ServerContext, ServerError, SocketPath, SystemProbe,
};

mod restore;

/// Runs a session until its last pane exits.
#[derive(Debug, Clone, Copy, Default)]
pub struct Server;

impl Server {
    /// Starts a tokio runtime and serves `config` on it.
    ///
    /// # Errors
    /// The socket or the initial panes could not be set up.
    pub fn run(config: ServerConfig) -> Result<(), ServerError> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("monarch-server")
            .build()
            .map_err(|e| ServerError::io("starting runtime", e))?
            .block_on(Self::serve(config))
    }

    /// Serves `config` on the current runtime.
    ///
    /// # Errors
    /// The socket or the initial panes could not be set up.
    pub async fn serve(config: ServerConfig) -> Result<(), ServerError> {
        let socket = match config.socket.clone() {
            Some(path) => path,
            None => SocketPath::for_session(&config.session)?,
        };
        let listener = bind(&config.session, &socket)?;
        let (events, events_rx) = mpsc::unbounded_channel();
        let env = PaneEnv {
            session: config.session.clone(),
            socket: socket.clone(),
        };
        let ctx = Arc::new(ServerContext::new(env, events, config.scrollback));
        restore::start_panes(&ctx, &config)?;
        let mut tasks = vec![
            tokio::spawn(FrameCoalescer::default().run(Arc::clone(&ctx))),
            tokio::spawn(EventLoop::run(Arc::clone(&ctx), events_rx)),
            tokio::spawn(CommandWatcher::new(Box::new(SystemProbe::new())).run(Arc::clone(&ctx))),
        ];
        if let Some(file) = config.state.clone() {
            let persister = Persister::new(file, Box::new(SystemProbe::new()));
            tasks.push(tokio::spawn(persister.run(Arc::clone(&ctx))));
        }
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    if let Ok((stream, _)) = accepted {
                        tokio::spawn(AttachConnection::serve(Arc::clone(&ctx), stream));
                    }
                }
                () = ctx.shutdown.notified() => break,
            }
        }
        tasks.iter().for_each(tokio::task::JoinHandle::abort);
        // Every pane exited on purpose: there is nothing to restore.
        if let Some(file) = &config.state {
            file.remove();
        }
        let _ = fs::remove_file(&socket);
        Ok(())
    }
}

/// Binds the socket, replacing a stale one but never a live server's.
fn bind(session: &str, path: &Path) -> Result<UnixListener, ServerError> {
    if path.exists() {
        if std::os::unix::net::UnixStream::connect(path).is_ok() {
            return Err(ServerError::AlreadyRunning {
                session: session.into(),
                socket: path.into(),
            });
        }
        fs::remove_file(path).map_err(|e| ServerError::io("removing stale socket", e))?;
    }
    let listener = UnixListener::bind(path)
        .map_err(|e| ServerError::io(format!("binding {}", path.display()), e))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| ServerError::io("securing socket", e))?;
    Ok(listener)
}
