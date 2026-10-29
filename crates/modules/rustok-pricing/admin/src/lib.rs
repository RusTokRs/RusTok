//! # rustok-pricing-admin
//!
//! Leptos admin UI package for the `rustok-pricing` module.
//!
//! Exposes pricing operations, price lists, discount adjustments, and channel-scoped
//! overrides for catalog products and variants.

#![recursion_limit = "256"]

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::PricingAdmin;

#[cfg(test)]
mod contract_tests;
