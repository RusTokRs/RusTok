//! # rustok-marketplace-listing-admin
//!
//! Leptos admin FFA package for the `rustok-marketplace-listing` module.
//!
//! Exposes administrative interfaces for marketplace product listings, seller
//! catalog assignments, commercial terms, and listing approvals.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use core::{
    MarketplaceListingAdminTransportProfile, build_marketplace_listing_admin_shell,
    selected_transport_profile,
};
pub use model::{
    MarketplaceListingAdminCommand, MarketplaceListingAdminCommandResult,
    MarketplaceListingAdminDetail, MarketplaceListingAdminDirectory,
    MarketplaceListingAdminFilters, MarketplaceListingAdminListItem,
    MarketplaceListingAdminRecord, MarketplaceListingAdminShell, MarketplaceListingAdminTerms,
};
pub use transport::{MarketplaceListingAdminTransportContext, load_listing_directory};
pub use ui::leptos::MarketplaceListingAdmin;

#[cfg(test)]
mod contract_tests;
