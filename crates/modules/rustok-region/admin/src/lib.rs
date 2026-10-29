//! # rustok-region-admin
//!
//! Leptos admin UI package for the `rustok-region` module.
//!
//! Exposes administrative interfaces for region management, country assignments,
//! currencies, and tax provider policy.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::RegionAdmin;

#[cfg(test)]
mod contract_tests;
