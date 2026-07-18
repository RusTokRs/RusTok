use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod receipts;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod service;

pub use dto::*;
pub use error::{MarketplaceAllocationError, MarketplaceAllocationResult};
pub use ports::*;
pub use service::MarketplaceAllocationService;

/// Owns the immutable seller and listing allocation of every marketplace order line.
pub struct MarketplaceAllocationModule;

#[async_trait]
impl RusToKModule for MarketplaceAllocationModule {
    fn slug(&self) -> &'static str {
        "marketplace_allocation"
    }

    fn name(&self) -> &'static str {
        "Marketplace Allocation"
    }

    fn description(&self) -> &'static str {
        "Marketplace Family order-line seller and listing allocation owner"
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

impl MigrationSource for MarketplaceAllocationModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
