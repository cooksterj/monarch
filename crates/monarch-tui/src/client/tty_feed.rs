//! The terminal's input, read by one thread for the life of the process
//! and handed to each attach in turn: switching sessions starts a new
//! input thread, and no key typed in between is lost.

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::Duration;

/// How often a waiting reader checks whether its attach has ended. Bytes
/// that arrive are returned at once; this bounds only the hand-over.
const STOP_CHECK: Duration = Duration::from_millis(50);

/// Chunks read from standard input, for whichever attach holds the lock.
static STDIN: OnceLock<Mutex<Receiver<Vec<u8>>>> = OnceLock::new();

/// One attach's hold on the input: reads until `stop` is set (then end of
/// input) or the terminal closes.
pub struct TtyFeed<'a> {
    chunks: MutexGuard<'a, Receiver<Vec<u8>>>,
    stop: Arc<AtomicBool>,
    left: Vec<u8>,
}

impl TtyFeed<'static> {
    /// Takes standard input, starting its reader thread on first use;
    /// waits while an earlier attach's thread still holds it.
    #[must_use]
    pub fn stdin(stop: Arc<AtomicBool>) -> Self {
        let chunks = STDIN.get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            let _ = thread::Builder::new()
                .name("monarch-tty".into())
                .spawn(move || pump(&mut io::stdin().lock(), &tx));
            Mutex::new(rx)
        });
        Self::new(chunks, stop)
    }
}

impl<'a> TtyFeed<'a> {
    /// Reads `chunks` until `stop` is set.
    pub fn new(chunks: &'a Mutex<Receiver<Vec<u8>>>, stop: Arc<AtomicBool>) -> Self {
        Self {
            chunks: chunks.lock().unwrap_or_else(PoisonError::into_inner),
            stop,
            left: Vec::new(),
        }
    }
}

impl Read for TtyFeed<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.left.is_empty() {
            if self.stop.load(Ordering::SeqCst) {
                return Ok(0);
            }
            match self.chunks.recv_timeout(STOP_CHECK) {
                Ok(chunk) => self.left = chunk,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
        let n = buf.len().min(self.left.len());
        buf[..n].copy_from_slice(&self.left[..n]);
        self.left.drain(..n);
        Ok(n)
    }
}

/// Copies `input` into `out` chunk by chunk until it ends.
fn pump(input: &mut impl Read, out: &mpsc::Sender<Vec<u8>>) {
    let mut buf = [0u8; 4096];
    while let Ok(n @ 1..) = input.read(&mut buf) {
        if out.send(buf[..n].to_vec()).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stopped_feed_hands_the_rest_to_the_next() {
        let (tx, rx) = mpsc::channel();
        let chunks = Mutex::new(rx);
        tx.send(b"ab".to_vec()).expect("send");
        let stop = Arc::new(AtomicBool::new(false));
        let mut first = TtyFeed::new(&chunks, Arc::clone(&stop));
        let mut buf = [0u8; 1];
        assert_eq!((first.read(&mut buf).ok(), buf), (Some(1), *b"a"));
        stop.store(true, Ordering::SeqCst);
        assert_eq!(first.read(&mut buf).ok(), Some(1), "what it holds is read");
        assert_eq!(first.read(&mut buf).ok(), Some(0), "then it ends");
        drop(first);
        tx.send(b"typed between".to_vec()).expect("send");
        let mut next = TtyFeed::new(&chunks, Arc::new(AtomicBool::new(false)));
        let mut out = [0u8; 32];
        let n = next.read(&mut out).expect("read");
        assert_eq!(&out[..n], b"typed between");
        drop(tx);
        assert_eq!(next.read(&mut out).ok(), Some(0), "terminal closed");
    }

    #[test]
    fn pump_copies_until_the_end() {
        let (tx, rx) = mpsc::channel();
        pump(&mut &b"hello"[..], &tx);
        assert_eq!(rx.try_recv().ok(), Some(b"hello".to_vec()));
    }
}
