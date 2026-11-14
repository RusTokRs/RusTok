//! # RusToK Product Storefront
//!
//! Leptos storefront UI package for the `rustok-product` module,
//! providing customer-facing catalog discovery, product detail views, and facet filtering.

// The hydrated buyer view's nested Leptos types exceed Rust's default layout depth.
#![recursion_limit = "256"]

mod catalog_controls;
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

#[cfg(test)]
mod contract_tests;

pub use model::{
    ProductCatalogFacet, ProductCatalogFacetValue, ProductCatalogSearchOption,
    ProductCatalogSearchOptions,
};
pub use transport::{fetch_catalog_facets, fetch_catalog_search_options};
pub use ui::leptos::CatalogFacetFilters;
pub use ui::leptos::ProductView;

#[cfg(target_arch = "wasm32")]
pub use ui::buyer::mount_product_buyer;
