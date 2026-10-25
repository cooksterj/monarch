//! Monarch agent-state detection: signals that say what an agent is
//! doing when its hooks don't. OSC notifications, and Herdr-compatible
//! screen manifests (rules over regions of the screen text).

mod detection_input;
mod error;
mod gate;
mod manifest;
mod manifest_registry;
mod osc_signal;
mod region;
mod rule;
mod rule_check;
mod verdict;

pub use detection_input::DetectionInput;
pub use error::DetectError;
pub use gate::Gate;
pub use manifest::{ENGINE_VERSION, Manifest};
pub use manifest_registry::ManifestRegistry;
pub use osc_signal::OscSignal;
pub use region::Region;
pub use rule::Rule;
pub use rule_check::RuleCheck;
pub use verdict::Verdict;
