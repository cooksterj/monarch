//! The session server process: socket, panes and background tasks.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use monarch_detect::ManifestRegistry;
use tokio::net::UnixListener;
use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;

use crate::{
    CliGitRunner, CommandWatcher, Connection, EventLoop, FrameCoalescer, PaneEnv, PaneEvent,
    Persister, PlaceWatcher, RepoProbe, ScreenWatcher, ServerConfig, ServerContext, ServerError,
    SocketPath, SystemProbe,
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
        let stop = Arc::new(Notify::new());
        let (tasks, persister) = background(&ctx, &config, events_rx, &stop);
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    if let Ok((stream, _)) = accepted {
                        tokio::spawn(Connection::serve(Arc::clone(&ctx), stream));
                    }
                }
                () = ctx.shutdown.notified() => break,
            }
        }
        // The persister finishes any save under way, then stops, so the
        // file deleted below stays deleted.
        stop.notify_one();
        tasks.iter().for_each(JoinHandle::abort);
        if let Some(persister) = persister {
            let _ = persister.await;
        }
        // Every pane exited on purpose: there is nothing to restore.
        if let Some(file) = &config.state {
            file.remove();
        }
        let _ = fs::remove_file(&socket);
        Ok(())
    }
}

/// Starts the session's long-running tasks: frames, pane events, the
/// watchers and, with a state file, the persister (returned apart: it is
/// stopped by `stop` and awaited, never aborted).
fn background(
    ctx: &Arc<ServerContext>,
    config: &ServerConfig,
    events: mpsc::UnboundedReceiver<PaneEvent>,
    stop: &Arc<Notify>,
) -> (Vec<JoinHandle<()>>, Option<JoinHandle<()>>) {
    let places = PlaceWatcher::new(
        Box::new(SystemProbe::new()),
        RepoProbe::new(Box::new(CliGitRunner)),
    );
    let tasks = vec![
        tokio::spawn(FrameCoalescer::default().run(Arc::clone(ctx))),
        tokio::spawn(EventLoop::run(Arc::clone(ctx), events)),
        tokio::spawn(CommandWatcher::new(Box::new(SystemProbe::new())).run(Arc::clone(ctx))),
        tokio::spawn(places.run(Arc::clone(ctx))),
        tokio::spawn(ScreenWatcher::new(ManifestRegistry::bundled()).run(Arc::clone(ctx))),
    ];
    let persister = config.state.clone().map(|file| {
        let persister = Persister::new(file, Box::new(SystemProbe::new()));
        tokio::spawn(persister.run(Arc::clone(ctx), Arc::clone(stop)))
    });
    (tasks, persister)
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
