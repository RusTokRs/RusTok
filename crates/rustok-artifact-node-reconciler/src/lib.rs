//! Independent deployment composition for authenticated artifact-node topology
//! reconciliation.
//!
//! The reconciler accepts one bounded mTLS operator request and delegates it
//! to the durable `rustok-modules` owner through the artifact-node transport.
//! It has no agent-claim listener, CAS, sandbox, product, AI, Alloy, tenant,
//! or application-server dependency.

mod config;

pub use config::ArtifactNodeReconcilerConfig;
