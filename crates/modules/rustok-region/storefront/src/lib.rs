//! # rustok-region-storefront
//!
//! Leptos storefront UI package for the `rustok-region` module.
//!
//! Exposes region discovery, country selection, currency presentation, and
//! regional tax baselines for shoppers.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::RegionView;

#[cfg(test)]
mod contract_tests;
