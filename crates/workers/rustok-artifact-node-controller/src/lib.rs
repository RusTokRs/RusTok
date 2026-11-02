//! Deployment composition for the owner-issued artifact-node agent port.

mod config;

pub use config::ArtifactNodeControllerConfig;

#[cfg(test)]
mod contract_tests;
