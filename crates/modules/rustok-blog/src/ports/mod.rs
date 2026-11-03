//! Blog cross-boundary ports and orchestration bridge types.
//!
//! Exposes owner in-transaction commands and types for content orchestration
//! (`rustok-content-orchestration`) and cross-module boundaries without leaking
//! private persistence entities.

pub mod orchestration;

pub use orchestration::*;
