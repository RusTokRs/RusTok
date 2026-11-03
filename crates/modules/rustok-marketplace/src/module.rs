//! # RusToK Marketplace Module Implementation
//!
//! Provides `MarketplaceModule` implementing `RusToKModule` and `MigrationSource`.

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

/// Marketplace family root.
///
/// This module composes marketplace owner modules and future cross-marketplace
/// workflows. It intentionally owns no seller, listing, allocation, commission,
/// ledger, or payout persistence.
pub struct MarketplaceModule;

#[async_trait]
impl RusToKModule for MarketplaceModule {
    fn slug(&self) -> &'static str {
        "marketplace"
    }

    fn name(&self) -> &'static str {
        "Marketplace"
    }

    fn description(&self) -> &'static str {
        "Marketplace family root and cross-marketplace orchestration boundary"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &[
            "marketplace_seller",
            "marketplace_listing",
            "marketplace_allocation",
            "marketplace_commission",
            "marketplace_balance",
            "marketplace_payout",
        ]
    }

    fn permissions(&self) -> Vec<Permission> {
        Vec::new()
    }
}

impl MigrationSource for MarketplaceModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}
