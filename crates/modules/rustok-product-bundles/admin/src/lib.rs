//! # rustok-product-bundles-admin
//!
//! Leptos admin UI package for the `rustok-product-bundles` module.
//!
//! Exposes administrative interfaces for bundle lifecycle management, component
//! items, kits, and configurable discounts.

#![recursion_limit = "256"]

pub mod core;
mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

pub use core::{
    BundleAdminTransportProfile, build_bundle_admin_shell, bundle_grid_columns, filter_bundles,
    matches_bundle_filter, selected_transport_profile, validate_bundle_discount,
    validate_bundle_name, validate_bundle_slug,
};
pub use model::{
    BundleAdminCommand, BundleAdminCommandResult, BundleAdminDirectory, BundleAdminFilters,
    BundleAdminItemRecord, BundleAdminListItem, BundleAdminRecord, BundleAdminTranslation,
};
pub use transport::{BundleAdminTransportContext, load_bundle_directory};
pub use ui::leptos::ProductBundlesAdmin;

#[cfg(test)]
mod contract_tests;
