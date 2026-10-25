//! Monarch agent-state detection: signals that say what an agent is
//! doing when its hooks don't (OSC notifications now; Herdr-compatible
//! screen manifests and process probes later).

mod osc_signal;

pub use osc_signal::OscSignal;
