use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod operation_orchestration;
pub mod provider_operation_journal;
pub mod provider_submission;
mod receipts;
#[cfg(test)]
mod tests;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod providers;
pub mod service;

pub use dto::*;
pub use error::{MarketplacePayoutError, MarketplacePayoutResult};
pub use ports::*;
pub use provider_operation_journal::*;
pub use provider_submission::*;
pub use providers::*;
pub use service::MarketplacePayoutService;

/// Owns seller payout scheduling and ledger-entry assignment.
pub struct MarketplacePayoutModule;

#[async_trait]
impl RusToKModule for MarketplacePayoutModule {
    fn slug(&self) -> &'static str {
        "marketplace_payout"
    }

    fn name(&self) -> &'static str {
        "Marketplace Payout"
    }

    fn description(&self) -> &'static str {
        "Marketplace Family seller payout scheduling owner"
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
            Permission::MARKETPLACE_SELLERS_MANAGE,
        ]
    }
}

impl MigrationSource for MarketplacePayoutModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
