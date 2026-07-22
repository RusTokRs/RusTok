use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod balance;
mod balance_transfer;
mod receipts;
mod reversal;
#[cfg(test)]
mod tests;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod service;

pub use dto::*;
pub use error::{MarketplaceLedgerError, MarketplaceLedgerResult};
pub use ports::*;
pub use service::MarketplaceLedgerService;

/// Owns immutable, balanced marketplace ledger transactions and entries.
pub struct MarketplaceLedgerModule;

#[async_trait]
impl RusToKModule for MarketplaceLedgerModule {
    fn slug(&self) -> &'static str {
        "marketplace_ledger"
    }

    fn name(&self) -> &'static str {
        "Marketplace Ledger"
    }

    fn description(&self) -> &'static str {
        "Marketplace Family immutable double-entry ledger owner"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::ORDERS_READ,
            Permission::ORDERS_LIST,
            Permission::ORDERS_MANAGE,
            Permission::MARKETPLACE_SELLERS_READ,
        ]
    }
}

impl MigrationSource for MarketplaceLedgerModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
