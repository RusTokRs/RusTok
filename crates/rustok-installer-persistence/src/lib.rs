//! SeaORM persistence adapter for durable installer state.

pub mod entities;
mod persistence;

pub use persistence::InstallerPersistenceService;
