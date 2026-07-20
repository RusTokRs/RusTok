use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::RusToKModule;

pub mod allocation_directory;
pub mod commission_directory;
pub mod financial_orchestration;
pub mod ledger_directory;
pub mod listing_directory;
pub mod seller_directory;

#[cfg(test)]
mod financial_orchestration_tests;

pub use allocation_directory::MarketplaceAllocationDirectoryService;
pub use commission_directory::MarketplaceCommissionDirectoryService;
pub use financial_orchestration::*;
pub use ledger_directory::MarketplaceLedgerDirectoryService;
pub use listing_directory::MarketplaceListingDirectoryService;
pub use seller_directory::MarketplaceSellerDirectoryService;

pub const MARKETPLACE_FAMILY_MODULES: &[&str] = &[
    "marketplace_seller",
    "marketplace_listing",
    "marketplace_allocation",
    "marketplace_commission",
    "marketplace_ledger",
    "marketplace_payout",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketplaceFamilyDescriptor {
    pub root_slug: &'static str,
    pub owner_modules: &'static [&'static str],
}

impl Default for MarketplaceFamilyDescriptor {
    fn default() -> Self {
        Self {
            root_slug: "marketplace",
            owner_modules: MARKETPLACE_FAMILY_MODULES,
        }
    }
}

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

    fn permissions(&self) -> Vec<Permission> {
        Vec::new()
    }
}
