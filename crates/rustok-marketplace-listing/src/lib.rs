use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

mod command_receipts;
#[cfg(test)]
mod command_receipts_tests;
mod evented_commands;
mod external_events;
mod lifecycle_event_commands;
mod listing_events;
mod replay_safe_commands;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod service;

pub use dto::*;
pub use entities::*;
pub use error::{MarketplaceListingError, MarketplaceListingResult};
pub use ports::*;
pub use service::MarketplaceListingService;

/// Marketplace Family owner for seller listing identity, versioned commercial
/// terms, lifecycle, approval, and deterministic eligibility projections.
pub struct MarketplaceListingModule;

#[async_trait]
impl RusToKModule for MarketplaceListingModule {
    fn slug(&self) -> &'static str {
        "marketplace_listing"
    }

    fn name(&self) -> &'static str {
        "Marketplace Listing"
    }

    fn description(&self) -> &'static str {
        "Marketplace Family listing identity, versioned terms, lifecycle, and eligibility owner"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::MARKETPLACE_LISTINGS_CREATE,
            Permission::MARKETPLACE_LISTINGS_READ,
            Permission::MARKETPLACE_LISTINGS_UPDATE,
            Permission::MARKETPLACE_LISTINGS_LIST,
            Permission::MARKETPLACE_LISTINGS_MANAGE,
            Permission::MARKETPLACE_LISTINGS_PUBLISH,
            Permission::MARKETPLACE_LISTINGS_MODERATE,
        ]
    }
}

impl MigrationSource for MarketplaceListingModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
