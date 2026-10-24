//! A protocol-level attach client that counts what it receives and can
//! stop reading on demand to play a stalled client.

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use bytes::BytesMut;
use monarch_proto::{ClientMsg, FrameCodec, ScreenSize, ServerMsg};
use monarch_tui::{ServerLink, TuiError};

/// Counters shared with the reader thread.
#[derive(Debug, Default)]
pub struct Counters {
    pub bytes: AtomicU64,
    pub snapshots: AtomicU64,
    pub deltas: AtomicU64,
    pub paused: AtomicBool,
}

/// One attached client.
pub struct BenchClient {
    pub link: ServerLink,
    pub counters: Arc<Counters>,
}

impl BenchClient {
    /// Connects, sizes the session and starts counting frames.
    pub fn connect(socket: &Path, size: ScreenSize) -> Result<Self, TuiError> {
        let (link, reader) = ServerLink::connect(socket)?;
        let counters = Arc::new(Counters::default());
        let shared = Arc::clone(&counters);
        thread::spawn(move || read_frames(reader, &shared));
        link.send(&ClientMsg::Resize(size))?;
        Ok(Self { link, counters })
    }

    pub fn bytes(&self) -> u64 {
        self.counters.bytes.load(Ordering::Relaxed)
    }

    pub fn snapshots(&self) -> u64 {
        self.counters.snapshots.load(Ordering::Relaxed)
    }

    pub fn set_paused(&self, paused: bool) {
        self.counters.paused.store(paused, Ordering::Relaxed);
    }
}

fn read_frames(mut stream: UnixStream, c: &Counters) {
    let codec = FrameCodec::<ServerMsg, ClientMsg>::default();
    let mut buf = BytesMut::new();
    let mut chunk = vec![0u8; 256 * 1024];
    loop {
        while c.paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
        }
        let n = match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        c.bytes.fetch_add(n as u64, Ordering::Relaxed);
        buf.extend_from_slice(&chunk[..n]);
        while let Ok(Some(msg)) = codec.decode_from(&mut buf) {
            match msg {
                ServerMsg::Snapshot(_) => c.snapshots.fetch_add(1, Ordering::Relaxed),
                ServerMsg::Delta(_) => c.deltas.fetch_add(1, Ordering::Relaxed),
                _ => 0,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn counts_bytes_and_waits_while_paused() {
        let (ours, mut theirs) = UnixStream::pair().expect("pair");
        let counters = Arc::new(Counters::default());
        counters.paused.store(true, Ordering::Relaxed);
        let shared = Arc::clone(&counters);
        let reader = thread::spawn(move || read_frames(ours, &shared));
        let mut frame = Vec::new();
        FrameCodec::<ClientMsg, ServerMsg>::default()
            .encode_to(&ServerMsg::Pong(1), &mut frame)
            .expect("encode");
        theirs.write_all(&frame).expect("write");
        thread::sleep(Duration::from_millis(50));
        assert_eq!(
            counters.bytes.load(Ordering::Relaxed),
            0,
            "paused: nothing read"
        );
        counters.paused.store(false, Ordering::Relaxed);
        drop(theirs);
        reader.join().expect("reader");
        assert_eq!(counters.bytes.load(Ordering::Relaxed), frame.len() as u64);
        assert_eq!(counters.snapshots.load(Ordering::Relaxed), 0);
    }
}
