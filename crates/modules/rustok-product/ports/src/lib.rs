//! # rustok-product-ports
//!
//! Canonical FBA ports, DTOs, and transport adapters for the RusTok Product module.

pub mod attached_values;
pub mod dto;
pub mod locale_replay;
pub mod ports;
pub mod types;
pub mod variant_replay;

#[cfg(any(feature = "grpc-client", feature = "grpc-server"))]
pub mod grpc;

pub use attached_values::{ProductAttachedValuesReadPort, ProductAttachedValuesWritePort};
pub use dto::*;
pub use locale_replay::*;
pub use ports::*;
pub use types::*;
pub use variant_replay::*;
