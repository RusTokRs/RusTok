//! # RusToK Marketplace
//!
//! Root and orchestration boundary for the RusToK Marketplace Family.
//! Composes marketplace owner modules (`marketplace_seller`, `marketplace_listing`, `marketplace_allocation`,
//! `marketplace_commission`, `marketplace_balance`, `marketplace_payout`) and drives cross-marketplace financial flows.

pub mod allocation_directory;
pub mod commission_directory;
pub mod financial_orchestration;
pub mod ledger_directory;
pub mod listing_directory;
pub mod module;
pub mod payout_directory;
pub mod seller_directory;

#[cfg(test)]
mod financial_orchestration_tests;

pub use allocation_directory::MarketplaceAllocationDirectoryService;
pub use commission_directory::MarketplaceCommissionDirectoryService;
pub use financial_orchestration::{
    MarketplaceFinancialCommandPort, MarketplaceFinancialOrchestrationError,
    MarketplaceFinancialOrchestrationResult, MarketplaceFinancialOrchestrationService,
    ProcessMarketplaceFinancialReversalInput, ProcessMarketplaceFinancialReversalResponse,
    ProcessMarketplaceOrderFinancialsInput, ProcessMarketplaceOrderFinancialsResponse,
};
pub use ledger_directory::MarketplaceLedgerDirectoryService;
pub use listing_directory::MarketplaceListingDirectoryService;
pub use module::MarketplaceModule;
pub use payout_directory::MarketplacePayoutDirectoryService;
pub use seller_directory::MarketplaceSellerDirectoryService;

pub const MARKETPLACE_FAMILY_MODULES: &[&str] = &[
    "marketplace_seller",
    "marketplace_listing",
    "marketplace_allocation",
    "marketplace_commission",
    "marketplace_balance",
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

#[cfg(test)]
mod contract_tests;
