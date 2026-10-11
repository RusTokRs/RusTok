//! SeaORM persistence adapter for durable installer state.

pub mod entities;
mod persistence;
mod seaorm_ports;

pub use entities::install_http_job::Model as InstallHttpJobModel;
pub use persistence::{InstallStepReceiptItem, InstallerPersistenceService};
pub use seaorm_ports::{
    SeaOrmInstallerApplyPorts, SeaOrmInstallerBootstrapPorts, SeaOrmInstallerPorts,
};
