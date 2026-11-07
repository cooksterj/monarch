//! The seam for reading the wall clock, so timing logic runs on a fake in
//! tests.

use crate::UnixMillis;

/// Something that tells the time. [`SystemClock`](crate::SystemClock)
/// reads the OS; tests pass a fake that stands still or jumps.
pub trait Clock: Send + Sync {
    /// The time now.
    fn now(&self) -> UnixMillis;
}
