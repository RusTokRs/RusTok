//! # RusToK Marketplace Listing Module Implementation
//!
//! Provides `MarketplaceListingModule` implementing `RusToKModule` and `MigrationSource`.

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

use crate::migrations;

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
        "Marketplace Family listing identity, versioned terms, lifecycle, approval, and eligibility owner"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["marketplace_seller", "product"]
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
