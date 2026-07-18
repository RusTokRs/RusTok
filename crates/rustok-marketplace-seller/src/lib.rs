use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod command_receipts;
mod localized_sellers;
mod receipted_commands;
mod seller_events;
#[cfg(test)]
mod seller_events_tests;

pub mod dto;
pub mod entities;
pub mod error;
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod migrations;
pub mod ports;
pub mod service;

pub use dto::*;
pub use entities::*;
pub use error::{MarketplaceSellerError, MarketplaceSellerResult};
pub use ports::*;
pub use service::MarketplaceSellerService;

/// Seller identity, lifecycle, onboarding, and membership owner for the
/// Marketplace Family.
pub struct MarketplaceSellerModule;

#[async_trait]
impl RusToKModule for MarketplaceSellerModule {
    fn slug(&self) -> &'static str {
        "marketplace_seller"
    }

    fn name(&self) -> &'static str {
        "Marketplace Seller"
    }

    fn description(&self) -> &'static str {
        "Marketplace Family seller identity, lifecycle, onboarding, and membership owner"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::MARKETPLACE_SELLERS_CREATE,
            Permission::MARKETPLACE_SELLERS_READ,
            Permission::MARKETPLACE_SELLERS_UPDATE,
            Permission::MARKETPLACE_SELLERS_DELETE,
            Permission::MARKETPLACE_SELLERS_LIST,
            Permission::MARKETPLACE_SELLERS_MANAGE,
        ]
    }
}

impl MigrationSource for MarketplaceSellerModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
