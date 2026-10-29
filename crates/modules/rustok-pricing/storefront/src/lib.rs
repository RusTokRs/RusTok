//! # rustok-pricing-storefront
//!
//! Leptos storefront UI package for the `rustok-pricing` module.
//!
//! Exposes public pricing discovery, price resolution context, and pricing views
//! for published catalog items.

mod core;
mod i18n;
mod model;
mod transport;

mod ui;

pub use ui::leptos::PricingView;

#[cfg(test)]
mod contract_tests;
