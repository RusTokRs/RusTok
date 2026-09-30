//! Artifact data infrastructure, storage brokers, and sandboxed capability adapters.

pub use crate::{ControlPlaneInfrastructure, ModuleCommandContext};

pub mod broker;
pub mod capabilities;
pub mod constants;
pub mod error;
pub mod export;
pub mod gc;
pub mod helpers;
pub mod object_capabilities;
pub mod objects;
pub mod objects_persistence;
pub mod purge;
pub mod purge_targets;
pub mod structured_persistence;
pub mod traits;
pub mod types;
pub mod upgrade;
pub mod upload;
pub mod upload_sessions;
pub mod validation;

pub use broker::*;
pub use capabilities::*;
pub use constants::*;
pub use error::*;
pub use export::*;
pub use gc::*;
pub use helpers::*;
pub use object_capabilities::*;
pub use objects::*;
pub use objects_persistence::*;
pub use purge::*;
pub use purge_targets::*;
pub use structured_persistence::*;
pub use traits::*;
pub use types::*;
pub use upgrade::*;
pub use upload::*;
pub use upload_sessions::*;
pub use validation::*;

#[cfg(test)]
mod tests;
