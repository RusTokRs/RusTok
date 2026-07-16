use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::RusToKModule;

pub const MARKETPLACE_FAMILY_MODULES: &[&str] = &[
    "marketplace_seller",
    "marketplace_listing",
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
/// workflows. It intentionally owns no seller, listing, commission, ledger, or
/// payout persistence.
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
