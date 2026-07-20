//! First-party Athanor integration for the RusToK AI retrieval boundary.
//!
//! The adapter intentionally keeps Athanor's embedded stores and search indexes behind
//! [`rustok_ai::RagRetrievalPort`] and [`rustok_ai::RagIngestionPort`]. The default crate build
//! contains only the boundary metadata; enable the `athanor` feature in the host that embeds the
//! Athanor runtime.

#[cfg(feature = "athanor")]
mod adapter;

#[cfg(feature = "athanor")]
pub use adapter::{AthanorRagAdapter, AthanorRagConfig, ATHANOR_SOURCE_ID};
