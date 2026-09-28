//! Owner-defined cross-boundary ports for the product catalog.
//!
//! This directory was created by splitting the former `ports.rs` monolith
//! into responsibility-oriented submodules per the canonical module layout
//! in `docs/backend/module-backend-implementation.md`.

mod catalog_read;
mod diagnostics;

pub use catalog_read::*;
