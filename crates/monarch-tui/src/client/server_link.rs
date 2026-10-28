//! The client end of the session socket.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use bytes::BytesMut;
use monarch_proto::{ClientMsg, FrameCodec, Hello, HelloAck, ServerMsg};

use super::ClientEvent;
use crate::TuiError;

type Codec = FrameCodec<ServerMsg, ClientMsg>;

/// Writes client messages to the socket from any thread. Cheap to clone;
/// writes are serialized by a mutex so frames never interleave.
#[derive(Debug, Clone)]
pub struct ServerLink {
    writer: Arc<Mutex<UnixStream>>,
    codec: Codec,
}

impl ServerLink {
    /// Connects and completes the handshake. Returns the link and a second
    /// handle on the same socket for [`Self::spawn_reader`].
    ///
    /// # Errors
    /// The socket is unreachable or the server refused to attach.
    pub fn connect(socket: &Path) -> Result<(Self, UnixStream), TuiError> {
        let io = |what: &str| {
            let context = format!("{what} {}", socket.display());
            move |e| TuiError::io(context, e)
        };
        let mut stream = UnixStream::connect(socket).map_err(io("connecting to"))?;
        let mut hello = serde_line(&Hello::attach(env!("CARGO_PKG_VERSION")));
        hello.push(b'\n');
        stream.write_all(&hello).map_err(io("greeting"))?;
        let ack = read_ack(&mut stream).map_err(io("reading handshake from"))?;
        if !ack.ok {
            return Err(TuiError::Refused(ack.error.unwrap_or_default()));
        }
        if ack.wire != Hello::WIRE {
            let session = socket.file_stem().unwrap_or_default().to_string_lossy();
            let reason = Hello::wire_mismatch(&session, ack.wire, Hello::WIRE);
            return Err(TuiError::OtherBuild(reason));
        }
        let reader = stream.try_clone().map_err(io("cloning"))?;
        let link = Self {
            writer: Arc::new(Mutex::new(stream)),
            codec: Codec::default(),
        };
        Ok((link, reader))
    }

    /// A link over an already-connected stream, for tests.
    #[cfg(test)]
    pub(crate) fn from_stream(stream: UnixStream) -> Self {
        Self {
            writer: Arc::new(Mutex::new(stream)),
            codec: Codec::default(),
        }
    }

    /// Sends one message.
    ///
    /// # Errors
    /// Encoding failed or the socket is closed.
    pub fn send(&self, msg: &ClientMsg) -> Result<(), TuiError> {
        let mut frame = Vec::new();
        self.codec.encode_to(msg, &mut frame)?;
        let mut writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        writer
            .write_all(&frame)
            .map_err(|e| TuiError::io("sending to server", e))
    }

    /// Closes the connection both ways: the server sees this client go,
    /// and the reader thread ends.
    pub fn close(&self) {
        let writer = self
            .writer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = writer.shutdown(std::net::Shutdown::Both);
    }

    /// Reads server frames on a std thread, forwarding each as a
    /// [`ClientEvent::Server`] and ending with [`ClientEvent::ServerClosed`].
    pub fn spawn_reader(mut stream: UnixStream, events: Sender<ClientEvent>) {
        let codec = Codec::default();
        let _ = thread::Builder::new()
            .name("monarch-link".into())
            .spawn(move || {
                let mut buf = BytesMut::with_capacity(64 * 1024);
                let mut chunk = vec![0u8; 64 * 1024];
                while let Ok(n @ 1..) = stream.read(&mut chunk) {
                    buf.extend_from_slice(&chunk[..n]);
                    while let Ok(Some(msg)) = codec.decode_from(&mut buf) {
                        if events.send(ClientEvent::Server(msg)).is_err() {
                            return;
                        }
                    }
                }
                let _ = events.send(ClientEvent::ServerClosed);
            });
    }
}

fn serde_line(hello: &Hello) -> Vec<u8> {
    serde_json::to_vec(hello).unwrap_or_default()
}

/// Reads the ack line byte by byte so no frame bytes are consumed.
fn read_ack(stream: &mut UnixStream) -> std::io::Result<HelloAck> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while line.len() < 4096 {
        stream.read_exact(&mut byte)?;
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
    }
    serde_json::from_slice(&line).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;

    use monarch_core::PaneId;

    use super::*;

    /// A fake server: accepts, checks the hello, acks, then echoes a Pong
    /// for the first Ping it receives.
    fn fake_server(socket: &Path, accept: bool) -> thread::JoinHandle<()> {
        let listener = UnixListener::bind(socket).expect("bind");
        thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            let mut line = Vec::new();
            let mut b = [0u8; 1];
            while s.read_exact(&mut b).is_ok() && b[0] != b'\n' {
                line.push(b[0]);
            }
            let hello: Hello = serde_json::from_slice(&line).expect("hello");
            assert!(hello.is_attach());
            let ack = if accept {
                HelloAck::accept("t")
            } else {
                HelloAck::refuse("t", "no")
            };
            let mut ack = serde_json::to_vec(&ack).expect("json");
            ack.push(b'\n');
            s.write_all(&ack).expect("ack");
            if !accept {
                return;
            }
            let codec = FrameCodec::<ClientMsg, ServerMsg>::default();
            let mut buf = BytesMut::new();
            let mut chunk = [0u8; 256];
            loop {
                // Several frames may arrive in one read: drain them all.
                while let Some(msg) = codec.decode_from(&mut buf).expect("frame") {
                    let ClientMsg::Ping(t) = msg else { continue };
                    let mut out = Vec::new();
                    codec
                        .encode_to(&ServerMsg::Pong(t), &mut out)
                        .expect("encode");
                    s.write_all(&out).expect("pong");
                    return;
                }
                let n = s.read(&mut chunk).expect("read");
                buf.extend_from_slice(&chunk[..n]);
            }
        })
    }

    #[test]
    fn handshake_send_and_receive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("s.sock");
        let server = fake_server(&socket, true);
        let (link, reader) = ServerLink::connect(&socket).expect("connect");
        let (tx, rx) = mpsc::channel();
        ServerLink::spawn_reader(reader, tx);
        link.send(&ClientMsg::Input {
            pane: PaneId(1),
            bytes: b"x".to_vec(),
        })
        .expect("input");
        link.send(&ClientMsg::Ping(42)).expect("ping");
        assert_eq!(
            rx.recv().expect("event"),
            ClientEvent::Server(ServerMsg::Pong(42))
        );
        server.join().expect("server");
        assert_eq!(rx.recv().expect("event"), ClientEvent::ServerClosed);
    }

    #[test]
    fn an_older_server_is_refused_with_how_to_restart_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("work.sock");
        let listener = UnixListener::bind(&socket).expect("bind");
        let server = thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            let mut b = [0u8; 1];
            while s.read_exact(&mut b).is_ok() && b[0] != b'\n' {}
            s.write_all(b"{\"ok\":true,\"server\":\"0.1.0\"}\n")
                .expect("ack");
        });
        let refused = ServerLink::connect(&socket);
        assert!(
            matches!(&refused, Err(TuiError::OtherBuild(r)) if r.contains("--session work$")),
            "{:?}",
            refused.err()
        );
        server.join().expect("server");
    }

    #[test]
    fn refusal_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let socket = dir.path().join("r.sock");
        let server = fake_server(&socket, false);
        assert!(matches!(ServerLink::connect(&socket), Err(TuiError::Refused(r)) if r == "no"));
        server.join().expect("server");
    }
}
