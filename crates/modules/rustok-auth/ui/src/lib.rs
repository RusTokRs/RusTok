//! # RusToK Auth UI
//!
//! Module-owned FFA client authentication boundary: framework-neutral session
//! policy, native/GraphQL transport facades, and Leptos lifecycle bindings.

pub mod core;
pub mod model;
pub mod transport;
pub mod ui;

#[cfg(test)]
mod contract_tests;
