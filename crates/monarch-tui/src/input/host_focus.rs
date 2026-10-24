//! Focus changes the host terminal reported.

/// The host window gained or lost focus (`CSI I` / `CSI O`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostFocus {
    /// The host window gained focus; a good moment to re-probe the host.
    Gained,
    /// The host window lost focus.
    Lost,
}
