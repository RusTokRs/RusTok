//! # rustok-installer-persistence
//!
//! SeaORM persistence adapter for durable installer state. It owns database
//! readiness, schema application, durable sessions, step receipts, and the
//! reusable installer ports shared by the HTTP and CLI wrappers.

pub mod entities;
mod persistence;
mod seaorm_ports;

pub use entities::install_http_job::Model as InstallHttpJobModel;
pub use persistence::{InstallStepReceiptItem, InstallerPersistenceService};
pub use seaorm_ports::{
    SeaOrmInstallerApplyPorts, SeaOrmInstallerBootstrapPorts, SeaOrmInstallerPorts,
};

#[cfg(test)]
mod contract_tests;
