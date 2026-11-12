//! # rustok-product-ports
//!
//! Canonical FBA ports, DTOs, and transport adapters for the RusTok Product module.

pub mod dto;
pub mod ports;
pub mod types;

#[cfg(any(feature = "grpc-client", feature = "grpc-server"))]
pub mod grpc;

pub use dto::*;
pub use ports::*;
pub use types::*;

