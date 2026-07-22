use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod allocation_filter;
#[cfg(test)]
mod cancelled_allocation_tests;
mod commission_service;
mod receipts;
mod service;
#[cfg(test)]
mod tests;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;

pub use commission_service::MarketplaceCommissionService;
pub use dto::*;
pub use error::{MarketplaceCommissionError, MarketplaceCommissionResult};
pub use ports::*;

/// Owns versioned commission rules and immutable assessments derived from allocations.
pub struct MarketplaceCommissionModule;

#[async_trait]
impl RusToKModule for MarketplaceCommissionModule {
    fn slug(&self) -> &'static str {
        "marketplace_commission"
    }

    fn name(&self) -> &'static str {
        "Marketplace Commission"
    }

    fn description(&self) -> &'static str {
        "Marketplace Family versioned commission rule and assessment owner"
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
            Permission::MARKETPLACE_LISTINGS_READ,
        ]
    }
}

impl MigrationSource for MarketplaceCommissionModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
