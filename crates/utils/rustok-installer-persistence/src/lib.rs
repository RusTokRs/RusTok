//! SeaORM persistence adapter for durable installer state.

pub mod entities;
mod persistence;
mod seaorm_ports;

pub use persistence::InstallerPersistenceService;
pub use seaorm_ports::{
    SeaOrmInstallerApplyPorts, SeaOrmInstallerBootstrapPorts, SeaOrmInstallerPorts,
};
